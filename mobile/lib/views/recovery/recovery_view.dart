import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';

import '../../l10n/strings.dart';
import '../../main.dart';
import '../../services/locator.dart';
import '../../services/recovery_service.dart';
import '../../theme/colors.dart';
import '../../utils/device_id.dart';
import '../../utils/secure_storage.dart';
import '../../widgets/cw_orb.dart';

/// Multi-step wallet recovery flow.
///
/// Stages:
///   1. email  — User enters email to receive OTP
///   2. otp    — User enters OTP code
///   3. backup — User imports backup shard (cloud or file)
///   4. recovering — Executing reshare protocol
///   5. done   — Success, navigate to home
enum _RecoveryStage { email, otp, backup, recovering, done }

class RecoveryView extends StatefulWidget {
  final String? initialEmail;

  const RecoveryView({super.key, this.initialEmail});

  @override
  State<RecoveryView> createState() => _RecoveryViewState();
}

class _RecoveryViewState extends State<RecoveryView> {
  _RecoveryStage _stage = _RecoveryStage.email;
  late final RecoveryService _recoveryService;

  // controllers
  final _emailCtrl = TextEditingController();
  final _otpCtrl = TextEditingController();

  // state
  BackupShardSource? _backupSource;
  bool _loading = false;
  String? _error;
  RecoveryVerifyResult? _verifyResult;

  @override
  void initState() {
    super.initState();
    _recoveryService = RecoveryService(Services.backup);
    if (widget.initialEmail != null && widget.initialEmail!.isNotEmpty) {
      _emailCtrl.text = widget.initialEmail!;
    }
  }

  @override
  void dispose() {
    _emailCtrl.dispose();
    _otpCtrl.dispose();
    _recoveryService.clearRecoveryState();
    super.dispose();
  }

  // ==================== Actions ====================

  Future<void> _submitEmail() async {
    final email = _emailCtrl.text.trim();
    if (email.isEmpty || !email.contains('@')) {
      setState(() => _error = S.recoveryEmailInvalid);
      return;
    }

    setState(() {
      _loading = true;
      _error = null;
    });

    try {
      await _recoveryService.initiateRecovery(email);
      if (!mounted) return;
      setState(() {
        _loading = false;
        _stage = _RecoveryStage.otp;
      });
    } on RecoveryException catch (e) {
      if (!mounted) return;
      setState(() {
        _loading = false;
        _error = e.message;
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _loading = false;
        _error = e.toString();
      });
    }
  }

  Future<void> _submitOtp() async {
    final otp = _otpCtrl.text.trim();
    if (otp.isEmpty || otp.length < 4) {
      setState(() => _error = S.recoveryOtpInvalid);
      return;
    }

    setState(() {
      _loading = true;
      _error = null;
    });

    try {
      final deviceId = await DeviceIdGenerator.getOrGenerate();
      final result = await _recoveryService.verifyOtp(
        otp: otp,
        deviceId: deviceId,
      );
      if (!mounted) return;
      setState(() {
        _loading = false;
        _verifyResult = result;
        _stage = _RecoveryStage.backup;
      });
    } on RecoveryException catch (e) {
      if (!mounted) return;
      setState(() {
        _loading = false;
        _error = e.message;
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _loading = false;
        _error = e.toString();
      });
    }
  }

  Future<void> _importFromCloud() async {
    setState(() {
      _loading = true;
      _error = null;
    });

    try {
      await _recoveryService.importBackupShard(
        source: BackupShardSource.cloud,
      );
      _backupSource = BackupShardSource.cloud;
      if (!mounted) return;
      await _executeRecovery();
    } on RecoveryException catch (e) {
      if (!mounted) return;
      setState(() {
        _loading = false;
        _error = _friendlyRecoveryError(e.message);
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _loading = false;
        _error = _friendlyRecoveryError(e.toString());
      });
    }
  }

  Future<void> _importFromFile() async {
    setState(() {
      _error = null;
    });

    try {
      final result = await FilePicker.platform.pickFiles(
        type: FileType.custom,
        allowedExtensions: ['json'],
      );

      if (result == null || result.files.isEmpty) return;

      final file = File(result.files.single.path!);
      final content = await file.readAsString();

      setState(() => _loading = true);

      await _recoveryService.importBackupShard(
        source: BackupShardSource.file,
        fileContent: content,
      );
      _backupSource = BackupShardSource.file;
      if (!mounted) return;
      await _executeRecovery();
    } on RecoveryException catch (e) {
      if (!mounted) return;
      setState(() {
        _loading = false;
        _error = _friendlyRecoveryError(e.message);
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _loading = false;
        _error = _friendlyRecoveryError(e.toString());
      });
    }
  }

  Future<void> _executeRecovery() async {
    if (_verifyResult == null) {
      setState(() => _error = 'Verification data missing');
      return;
    }

    setState(() {
      _stage = _RecoveryStage.recovering;
      _loading = true;
      _error = null;
    });

    try {
      final result = await _recoveryService.executeRecovery(
        publicKeyHex: _verifyResult!.publicKeyHex,
        serverReshareMessagesJson: _verifyResult!.serverReshareMessagesJson,
        serverCommitmentHex: _verifyResult!.serverCommitmentHex,
      );

      if (!mounted) return;

      // Update app state with recovered wallet
      final appState = CowalletApp.of(context);
      appState.setWalletAddress(result.address);
      appState.completeOnboarding();

      // Clear persisted onboarding progress so "未完成" banner disappears
      await SecureStorage.delete(SecureStorage.keyOnboardingStep);
      await SecureStorage.delete(SecureStorage.keyPendingBackupCreatedAt);
      await SecureStorage.save('onboarding_completed_at', DateTime.now().toIso8601String());
      await SecureStorage.save('backup_status', 'recovered');
      await SecureStorage.save('mpc_address', result.address);

      // Persist public key and server commitment for key health verification
      final pubKeyHex = result.publicKey.map((b) => b.toRadixString(16).padLeft(2, '0')).join();
      await SecureStorage.save('mpc_public_key', pubKeyHex);
      await SecureStorage.save('mpc_server_commitment', _verifyResult!.serverCommitmentHex);

      // Persist backup shard method and last-checked for key health service
      final addrSuffix = result.address.toLowerCase().substring(0, 10);
      final methodStr = _backupSource == BackupShardSource.cloud ? 'cloud' : 'file';
      await SecureStorage.save('backup_shard_method_$addrSuffix', methodStr);
      await SecureStorage.save('key_backup_last_checked_$addrSuffix', DateTime.now().toIso8601String());

      setState(() {
        _loading = false;
        _stage = _RecoveryStage.done;
      });

      // Refresh balance in background
      Services.balance.refresh(result.address);
    } on RecoveryException catch (e) {
      if (!mounted) return;
      setState(() {
        _loading = false;
        _stage = _RecoveryStage.backup;
        _error = _friendlyRecoveryError(e.message);
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _loading = false;
        _stage = _RecoveryStage.backup;
        _error = _friendlyRecoveryError(e.toString());
      });
    }
  }

  String _friendlyRecoveryError(String raw) {
    if (raw.contains('commitment verification failed') ||
        raw.contains('backup shard is incorrect')) {
      return '备份密钥验证失败，请确认您导入的是注册时备份的正确密钥文件。';
    }
    if (raw.contains('not a valid scalar') ||
        raw.contains('invalid backup shard')) {
      return '备份密钥格式无效，请检查文件是否完整。';
    }
    if (raw.contains('No backup shard found')) {
      return '未找到云端备份，请尝试从文件导入。';
    }
    return raw;
  }

  void _navigateHome() {
    Navigator.pushReplacementNamed(context, '/');
  }

  // ==================== Build ====================

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: CwColors.bgPaper,
      body: SafeArea(
        child: AnimatedSwitcher(
          duration: const Duration(milliseconds: 300),
          child: _buildStage(),
        ),
      ),
    );
  }

  Widget _buildStage() {
    switch (_stage) {
      case _RecoveryStage.email:
        return _emailStage();
      case _RecoveryStage.otp:
        return _otpStage();
      case _RecoveryStage.backup:
        return _backupStage();
      case _RecoveryStage.recovering:
        return _recoveringStage();
      case _RecoveryStage.done:
        return _doneStage();
    }
  }

  // ==================== Shared Widgets ====================

  Widget _topBar({bool showBack = true}) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 12),
      child: Row(
        children: [
          if (showBack)
            GestureDetector(
              onTap: _handleBack,
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Icon(Icons.arrow_back_ios_new, size: 16, color: CwColors.ink3),
                  const SizedBox(width: 4),
                  Text(S.back, style: TextStyle(fontSize: 14, color: CwColors.ink3)),
                ],
              ),
            )
          else
            const SizedBox(width: 48),
          const Spacer(),
          const SizedBox(width: 48),
        ],
      ),
    );
  }

  void _handleBack() {
    switch (_stage) {
      case _RecoveryStage.otp:
        setState(() {
          _stage = _RecoveryStage.email;
          _error = null;
        });
        break;
      case _RecoveryStage.backup:
        setState(() {
          _stage = _RecoveryStage.otp;
          _error = null;
        });
        break;
      default:
        Navigator.pop(context);
    }
  }

  Widget _heading(String text) {
    return Text(
      text,
      style: Theme.of(context).textTheme.displayMedium,
      textAlign: TextAlign.center,
    );
  }

  Widget _subtitle(String text) {
    return Text(
      text,
      style: Theme.of(context).textTheme.bodyLarge?.copyWith(color: CwColors.ink2),
      textAlign: TextAlign.center,
    );
  }

  Widget _primaryButton(String label, VoidCallback? onPressed) {
    return SizedBox(
      width: double.infinity,
      child: FilledButton(
        onPressed: _loading ? null : onPressed,
        child: _loading
            ? const SizedBox(
                width: 20,
                height: 20,
                child: CircularProgressIndicator(strokeWidth: 2, color: Colors.white),
              )
            : Text(label),
      ),
    );
  }

  Widget _errorBanner() {
    if (_error == null) return const SizedBox.shrink();
    return Container(
      width: double.infinity,
      padding: const EdgeInsets.all(14),
      margin: const EdgeInsets.only(bottom: 16),
      decoration: BoxDecoration(
        color: CwColors.dangerSoft,
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: CwColors.danger.withValues(alpha: 0.3)),
      ),
      child: Row(
        children: [
          Icon(Icons.error_outline, size: 20, color: CwColors.danger),
          const SizedBox(width: 10),
          Expanded(
            child: Text(
              _error!,
              style: TextStyle(fontSize: 13, color: CwColors.ink2),
            ),
          ),
        ],
      ),
    );
  }

  // ==================== Stage 1: Email ====================

  Widget _emailStage() {
    return SingleChildScrollView(
      key: const ValueKey('email'),
      child: Column(
        children: [
          _topBar(),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 28),
            child: Column(
              children: [
                const SizedBox(height: 24),
                Icon(Icons.restore, size: 64, color: CwColors.accent),
                const SizedBox(height: 24),
                _heading(S.recoveryH1),
                const SizedBox(height: 8),
                _subtitle(S.recoverySub),
                const SizedBox(height: 32),
                _errorBanner(),
                Container(
                  decoration: BoxDecoration(
                    color: CwColors.bgCard,
                    borderRadius: BorderRadius.circular(14),
                    border: Border.all(color: CwColors.line),
                  ),
                  child: TextField(
                    controller: _emailCtrl,
                    keyboardType: TextInputType.emailAddress,
                    autocorrect: false,
                    style: const TextStyle(fontSize: 16, color: CwColors.ink1),
                    decoration: InputDecoration(
                      hintText: S.recoveryEmailHint,
                      hintStyle: TextStyle(fontSize: 16, color: CwColors.ink4),
                      contentPadding: const EdgeInsets.symmetric(horizontal: 16, vertical: 18),
                      border: InputBorder.none,
                    ),
                    onSubmitted: (_) => _submitEmail(),
                  ),
                ),
                const SizedBox(height: 24),
                _primaryButton(S.recoverySendOtp, _submitEmail),
                const SizedBox(height: 16),
                TextButton(
                  onPressed: () => Navigator.pop(context),
                  child: Text(
                    S.recoveryCancel,
                    style: TextStyle(color: CwColors.ink3, fontSize: 14),
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  // ==================== Stage 2: OTP ====================

  Widget _otpStage() {
    return SingleChildScrollView(
      key: const ValueKey('otp'),
      child: Column(
        children: [
          _topBar(),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 28),
            child: Column(
              children: [
                const SizedBox(height: 24),
                Icon(Icons.mark_email_read_outlined, size: 64, color: CwColors.accent),
                const SizedBox(height: 24),
                _heading(S.recoveryOtpH1),
                const SizedBox(height: 8),
                _subtitle(S.recoveryOtpSub),
                const SizedBox(height: 32),
                _errorBanner(),
                Container(
                  decoration: BoxDecoration(
                    color: CwColors.bgCard,
                    borderRadius: BorderRadius.circular(14),
                    border: Border.all(color: CwColors.line),
                  ),
                  child: TextField(
                    controller: _otpCtrl,
                    keyboardType: TextInputType.number,
                    textAlign: TextAlign.center,
                    style: const TextStyle(
                      fontSize: 24,
                      fontWeight: FontWeight.w600,
                      letterSpacing: 8,
                      color: CwColors.ink1,
                    ),
                    decoration: InputDecoration(
                      hintText: '------',
                      hintStyle: TextStyle(
                        fontSize: 24,
                        fontWeight: FontWeight.w400,
                        letterSpacing: 8,
                        color: CwColors.ink4,
                      ),
                      contentPadding: const EdgeInsets.symmetric(horizontal: 16, vertical: 18),
                      border: InputBorder.none,
                    ),
                    onSubmitted: (_) => _submitOtp(),
                  ),
                ),
                const SizedBox(height: 24),
                _primaryButton(S.recoveryVerify, _submitOtp),
              ],
            ),
          ),
        ],
      ),
    );
  }

  // ==================== Stage 3: Backup Import ====================

  Widget _backupStage() {
    return SingleChildScrollView(
      key: const ValueKey('backup'),
      child: Column(
        children: [
          _topBar(),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 28),
            child: Column(
              children: [
                const SizedBox(height: 24),
                Icon(Icons.vpn_key_outlined, size: 64, color: CwColors.accent),
                const SizedBox(height: 24),
                _heading(S.recoveryBackupH1),
                const SizedBox(height: 8),
                _subtitle(S.recoveryBackupSub),
                const SizedBox(height: 32),
                _errorBanner(),
                if (_loading) ...[
                  const CircularProgressIndicator(),
                  const SizedBox(height: 16),
                  Text(
                    S.recoveryImporting,
                    style: TextStyle(color: CwColors.ink3),
                  ),
                ] else ...[
                  _backupOptionCard(
                    icon: Icons.cloud_download_outlined,
                    title: S.recoveryFromCloud,
                    desc: S.recoveryFromCloudDesc,
                    onTap: _importFromCloud,
                  ),
                  const SizedBox(height: 12),
                  _backupOptionCard(
                    icon: Icons.file_open_outlined,
                    title: S.recoveryFromFile,
                    desc: S.recoveryFromFileDesc,
                    onTap: _importFromFile,
                  ),
                ],
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _backupOptionCard({
    required IconData icon,
    required String title,
    required String desc,
    required VoidCallback onTap,
  }) {
    return GestureDetector(
      onTap: onTap,
      child: Container(
        width: double.infinity,
        padding: const EdgeInsets.all(16),
        decoration: BoxDecoration(
          color: CwColors.bgCard,
          borderRadius: BorderRadius.circular(16),
          border: Border.all(color: CwColors.line),
        ),
        child: Row(
          children: [
            Container(
              width: 44,
              height: 44,
              decoration: BoxDecoration(
                color: CwColors.accentSoft,
                borderRadius: BorderRadius.circular(12),
              ),
              child: Icon(icon, size: 22, color: CwColors.accent),
            ),
            const SizedBox(width: 14),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    title,
                    style: TextStyle(
                      fontSize: 15,
                      fontWeight: FontWeight.w600,
                      color: CwColors.ink1,
                    ),
                  ),
                  const SizedBox(height: 2),
                  Text(desc, style: TextStyle(fontSize: 13, color: CwColors.ink3)),
                ],
              ),
            ),
            Icon(Icons.chevron_right, size: 20, color: CwColors.ink4),
          ],
        ),
      ),
    );
  }

  // ==================== Stage 4: Recovering ====================

  Widget _recoveringStage() {
    return SingleChildScrollView(
      key: const ValueKey('recovering'),
      child: Column(
        children: [
          _topBar(showBack: false),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 28),
            child: Column(
              children: [
                const SizedBox(height: 40),
                const CwOrb(size: 120, thinking: true),
                const SizedBox(height: 28),
                _heading(S.recoveryInProgress),
                const SizedBox(height: 8),
                _subtitle(S.recoveryInProgressSub),
                const SizedBox(height: 32),
                _checkLine(S.recoveryStep1, true),
                const SizedBox(height: 12),
                _checkLine(S.recoveryStep2, true),
                const SizedBox(height: 12),
                _checkLine(S.recoveryStep3, false),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _checkLine(String text, bool done) {
    return Row(
      children: [
        AnimatedSwitcher(
          duration: const Duration(milliseconds: 300),
          child: done
              ? Icon(Icons.check_circle,
                  key: ValueKey('$text-done'), size: 20, color: CwColors.success)
              : SizedBox(
                  key: ValueKey('$text-wait'),
                  width: 20,
                  height: 20,
                  child: CircularProgressIndicator(strokeWidth: 2, color: CwColors.ink4),
                ),
        ),
        const SizedBox(width: 12),
        Expanded(
          child: Text(
            text,
            style: Theme.of(context).textTheme.bodyLarge?.copyWith(
                  color: done ? CwColors.ink1 : CwColors.ink3,
                ),
          ),
        ),
      ],
    );
  }

  // ==================== Stage 5: Done ====================

  Widget _doneStage() {
    return SingleChildScrollView(
      key: const ValueKey('done'),
      child: Column(
        children: [
          _topBar(showBack: false),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 28),
            child: Column(
              children: [
                const SizedBox(height: 40),
                SizedBox(
                  width: 140,
                  height: 140,
                  child: Stack(
                    alignment: Alignment.center,
                    children: [
                      const CwOrb(size: 120, breathing: true),
                      Positioned(
                        right: 8,
                        bottom: 8,
                        child: Container(
                          width: 36,
                          height: 36,
                          decoration: BoxDecoration(
                            color: CwColors.success,
                            shape: BoxShape.circle,
                            border: Border.all(color: CwColors.bgPaper, width: 3),
                          ),
                          child: const Icon(Icons.check, size: 20, color: Colors.white),
                        ),
                      ),
                    ],
                  ),
                ),
                const SizedBox(height: 28),
                _heading(S.recoveryDoneH1),
                const SizedBox(height: 8),
                _subtitle(S.recoveryDoneSub),
                const SizedBox(height: 40),
                _primaryButton(S.recoveryGoHome, _navigateHome),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
