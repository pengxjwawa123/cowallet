import 'dart:io';

import 'package:flutter/material.dart';
import 'package:file_picker/file_picker.dart';
import '../../theme/colors.dart';
import '../../l10n/strings.dart';
import '../../widgets/cw_chip.dart';
import '../../widgets/top_toast.dart';
import '../../services/locator.dart';
import '../../services/key_health_service.dart';
import '../../services/backup_shard_service.dart';
import '../../utils/secure_storage.dart';

class KeysView extends StatefulWidget {
  const KeysView({super.key});

  @override
  State<KeysView> createState() => _KeysViewState();
}

class _KeysViewState extends State<KeysView> {
  bool _isAuthenticated = false;
  bool _testingBackup = false;
  bool _phoneLoading = true;
  bool _serverLoading = true;
  bool _backupLoading = true;

  final _keyHealth = KeyHealthService();
  KeyHealth? _phoneHealth;
  KeyHealth? _serverHealth;
  KeyHealth? _backupHealth;
  BackupMethod? _backupMethod;

  @override
  void initState() {
    super.initState();
    _init();
  }

  Future<void> _init() async {
    await _checkBiometricStatus();
    await _runHealthChecks();
  }

  Future<void> _checkBiometricStatus() async {
    final available = await Services.biometrics.isAvailable();
    final enabled = await Services.biometrics.isEnabled();
    final hasEnrolled = await Services.biometrics.hasEnrolledBiometrics();
    if (mounted) {
      if (!available || !enabled || !hasEnrolled) {
        setState(() => _isAuthenticated = true);
      }
    }
  }

  Future<void> _runHealthChecks() async {
    setState(() {
      _phoneLoading = true;
      _serverLoading = true;
      _backupLoading = true;
    });

    _keyHealth.getBackupMethod().then((m) {
      if (mounted) setState(() => _backupMethod = m);
    });
    _keyHealth.checkPhoneKey().then((h) {
      if (mounted) setState(() { _phoneHealth = h; _phoneLoading = false; });
      _saveStatus('key_phone_status', h.status);
    });
    _keyHealth.checkServerKey().then((h) {
      if (mounted) setState(() { _serverHealth = h; _serverLoading = false; });
      _saveStatus('key_server_status', h.status);
    });
    _keyHealth.checkBackupKey().then((h) {
      if (mounted) setState(() { _backupHealth = h; _backupLoading = false; });
      _saveStatus('key_backup_status', h.status);
    });
  }

  Future<bool> _authenticate() async {
    if (_isAuthenticated) return true;

    final authenticated = await Services.authenticate(reason: S.biometricAuthReason);

    if (authenticated && mounted) {
      setState(() => _isAuthenticated = true);
    }
    return authenticated;
  }

  Future<void> _testBackupKey() async {
    setState(() => _testingBackup = true);

    bool? success;
    if (_backupMethod == BackupMethod.file) {
      success = await _testBackupKeyWithFile();
    } else {
      success = await _keyHealth.testBackupKey();
    }

    if (mounted) {
      setState(() => _testingBackup = false);
      if (success == null) return; // user cancelled file picker
      if (success) {
        _backupHealth = KeyHealth(
          status: KeyStatus.ok,
          lastChecked: DateTime.now(),
        );
        setState(() {});
        _saveStatus('key_backup_status', KeyStatus.ok);
        showTopToast(context, S.backupTestSuccess, backgroundColor: CwColors.success);
      } else {
        _backupHealth = KeyHealth(
          status: KeyStatus.error,
          lastChecked: null,
          error: S.backupTestFailed,
        );
        setState(() {});
        _saveStatus('key_backup_status', KeyStatus.error);
        _clearBackupLastChecked();
        showTopToast(context, S.backupTestFailed, backgroundColor: CwColors.danger);
      }
    }
  }

  Future<bool?> _testBackupKeyWithFile() async {
    try {
      final result = await FilePicker.platform.pickFiles(
        type: FileType.custom,
        allowedExtensions: ['json'],
      );
      if (result == null || result.files.isEmpty) return null;

      final filePath = result.files.single.path;
      if (filePath == null) return null;

      final fileContent = await File(filePath).readAsString();
      return await _keyHealth.testBackupKeyWithFile(fileContent);
    } catch (_) {
      return false;
    }
  }

  Future<void> _saveStatus(String prefix, KeyStatus status) async {
    final addr = await SecureStorage.get('mpc_address');
    final suffix = (addr != null && addr.length >= 10) ? addr.toLowerCase().substring(0, 10) : 'unknown';
    await SecureStorage.save('${prefix}_$suffix', status.name);
  }

  Future<void> _clearBackupLastChecked() async {
    final addr = await SecureStorage.get('mpc_address');
    final suffix = (addr != null && addr.length >= 10) ? addr.toLowerCase().substring(0, 10) : 'unknown';
    await SecureStorage.delete('key_backup_last_checked_$suffix');
  }

  String _formatTimeAgo(DateTime? time) {
    if (time == null) return '—';
    final diff = DateTime.now().difference(time);
    if (diff.inMinutes < 1) return S.justNow;
    if (diff.inMinutes < 60) return S.minutesAgo(diff.inMinutes);
    if (diff.inHours < 24) return S.hoursAgo(diff.inHours);
    return S.daysAgo(diff.inDays);
  }

  String _phoneMetaText() {
    if (_phoneHealth == null) return '...';
    if (_phoneHealth!.status == KeyStatus.error) return '✗ ${_phoneHealth!.error ?? S.keyUnavailable}';
    final lastUsed = _phoneHealth!.lastUsed;
    if (lastUsed != null) return '✓ ${S.keyIntact} · ${S.keyLastUsed(_formatTimeAgo(lastUsed))}';
    return '✓ ${S.keyIntact}';
  }

  String _serverMetaText() {
    if (_serverHealth == null) return '...';
    if (_serverHealth!.status == KeyStatus.error) return '✗ ${S.keyServerUnreachable}';
    if (_serverHealth!.status == KeyStatus.warning) return '⚠ ${_serverHealth!.error ?? S.keyServerWarning}';
    final checked = _serverHealth!.lastChecked;
    return '✓ ${S.keyHeartbeat(_formatTimeAgo(checked))}';
  }

  String _backupMetaText() {
    if (_backupHealth == null) return '...';
    if (_backupHealth!.status == KeyStatus.error) return '✗ ${S.backupTestFailed}';
    if (_backupHealth!.status == KeyStatus.ok) {
      final lastChecked = _backupHealth!.lastChecked;
      if (lastChecked != null) {
        final days = DateTime.now().difference(lastChecked).inDays;
        if (days >= KeyHealthService.verifyExpiryDays) return '⚠ ${S.keyNotVerifiedDays(days)}';
      }
      return '✓ ${S.keyVerified(_formatTimeAgo(_backupHealth!.lastChecked))}';
    }
    if (_backupHealth!.status == KeyStatus.warning) return '⚠ ${S.keyNotVerified}';
    return '⚠ ${S.keyNotVerified}';
  }

  KeyStatus _overallBackupStatus() {
    if (_backupHealth == null) return KeyStatus.unknown;
    if (_backupHealth!.status == KeyStatus.ok && _backupHealth!.lastChecked != null) {
      final days = DateTime.now().difference(_backupHealth!.lastChecked!).inDays;
      if (days >= KeyHealthService.verifyExpiryDays) return KeyStatus.warning;
    }
    return _backupHealth!.status;
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        leading: const BackButton(),
      ),
      body: RefreshIndicator(
        onRefresh: _runHealthChecks,
        child: ListView(
          padding: const EdgeInsets.fromLTRB(20, 0, 20, 40),
          children: [
            // Hero
            Text.rich(
              TextSpan(
                children: [
                  TextSpan(
                    text: '${S.keysH1a}\n',
                    style: const TextStyle(
                      fontFamily: 'NotoSerifSC',
                      fontSize: 26,
                      fontWeight: FontWeight.w600,
                      color: CwColors.ink1,
                      letterSpacing: -0.52,
                      height: 1.3,
                    ),
                  ),
                  TextSpan(
                    text: S.keysH1b,
                    style: const TextStyle(
                      fontFamily: 'NotoSerifSC',
                      fontSize: 26,
                      fontWeight: FontWeight.w600,
                      color: CwColors.ink1,
                      letterSpacing: -0.52,
                      height: 1.3,
                    ),
                  ),
                  TextSpan(
                    text: S.keysH1em,
                    style: const TextStyle(
                      fontFamily: 'Fraunces',
                      fontSize: 26,
                      fontWeight: FontWeight.w600,
                      fontStyle: FontStyle.italic,
                      color: CwColors.accent,
                      letterSpacing: -0.52,
                      height: 1.3,
                    ),
                  ),
                ],
              ),
            ),
            const SizedBox(height: 8),
            Text(
              S.keysSub,
              style: const TextStyle(fontSize: 13, color: CwColors.ink3, height: 1.5),
            ),
            const SizedBox(height: 20),

            // Key 1: Phone
            _keyCard(
              icon: Icons.phone_iphone,
              status: _phoneHealth?.status ?? KeyStatus.unknown,
              title: S.keyPhone,
              where: S.keyPhoneWhere,
              meta: _isAuthenticated ? _phoneMetaText() : '••••••••',
              loading: _phoneLoading,
            ),
            const SizedBox(height: 10),

            // Key 2: Server
            _keyCard(
              icon: Icons.dns_outlined,
              status: _serverHealth?.status ?? KeyStatus.unknown,
              title: S.keyCloud,
              where: S.keyCloudWhere,
              meta: _isAuthenticated ? _serverMetaText() : '••••••••',
              loading: _serverLoading,
            ),
            const SizedBox(height: 10),

            // Key 3: Backup
            _keyCard(
              icon: Icons.lock_outline,
              status: _overallBackupStatus(),
              title: S.keyRecovery,
              where: _backupMethod == BackupMethod.file ? S.keyRecoveryWhereFile : S.keyRecoveryWhere,
              meta: _isAuthenticated ? _backupMetaText() : '••••••••',
              actionLabel: _isAuthenticated
                  ? (_backupMethod == BackupMethod.file ? S.keyRecoveryActionFile : S.keyRecoveryAction)
                  : null,
              onAction: _testBackupKey,
              actionLoading: _testingBackup,
              loading: _backupLoading,
            ),
            const SizedBox(height: 24),

            // Explainer card
            Container(
              padding: const EdgeInsets.all(14),
              decoration: BoxDecoration(
                color: CwColors.bgSubtle,
                borderRadius: BorderRadius.circular(14),
              ),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    S.keysExplainLabel,
                    style: const TextStyle(
                      fontFamily: 'NotoSerifSC',
                      fontSize: 13.5,
                      fontWeight: FontWeight.w600,
                      color: CwColors.ink1,
                    ),
                  ),
                  const SizedBox(height: 6),
                  Text(
                    S.keysExplainBody,
                    style: const TextStyle(fontSize: 12, color: CwColors.ink3, height: 1.6),
                  ),
                ],
              ),
            ),
            const SizedBox(height: 12),

            // Tech detail card
            Container(
              padding: const EdgeInsets.all(14),
              decoration: BoxDecoration(
                color: CwColors.bgCard,
                borderRadius: BorderRadius.circular(14),
                border: Border.all(color: CwColors.line),
              ),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Row(
                    children: [
                      Text(
                        S.keysTechLabel,
                        style: const TextStyle(
                          fontFamily: 'NotoSerifSC',
                          fontSize: 13.5,
                          fontWeight: FontWeight.w600,
                          color: CwColors.ink1,
                        ),
                      ),
                      const SizedBox(width: 8),
                      const CwChip(
                        label: 'MPC 2-of-3',
                        variant: ChipVariant.info,
                        fontSize: 10,
                      ),
                    ],
                  ),
                  const SizedBox(height: 6),
                  Text(
                    S.keysTechBody,
                    style: const TextStyle(fontSize: 12, color: CwColors.ink3, height: 1.6),
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _keyCard({
    required IconData icon,
    required KeyStatus status,
    required String title,
    required String where,
    required String meta,
    String? actionLabel,
    VoidCallback? onAction,
    bool actionLoading = false,
    bool loading = false,
  }) {
    final isOk = status == KeyStatus.ok && !loading;
    final isWarn = status == KeyStatus.warning && !loading;
    final statusColor = loading ? CwColors.ink4 : (isOk ? CwColors.success : (isWarn ? CwColors.warn : CwColors.danger));
    final bgColor = loading ? CwColors.bgCard : (isOk ? CwColors.successSoft : (isWarn ? CwColors.warnSoft : const Color(0xFFFDE8E8)));
    final borderColor = loading ? CwColors.line : (isOk ? const Color(0xFFC9D7BC) : (isWarn ? const Color(0xFFE4D2A8) : const Color(0xFFE8BFBF)));
    final chipLabel = isOk ? 'OK' : (isWarn ? S.keyRecoveryTag : '!');
    final chipVariant = isOk ? ChipVariant.green : (isWarn ? ChipVariant.amber : ChipVariant.amber);

    return GestureDetector(
      onTap: !_isAuthenticated ? () => _authenticate() : null,
      child: Container(
        padding: const EdgeInsets.all(14),
        decoration: BoxDecoration(
          color: bgColor,
          borderRadius: BorderRadius.circular(14),
          border: Border.all(color: borderColor),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Container(
                  width: 40,
                  height: 40,
                  decoration: BoxDecoration(
                    color: bgColor,
                    borderRadius: BorderRadius.circular(10),
                  ),
                  child: Icon(icon, size: 20, color: statusColor),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        title,
                        style: const TextStyle(
                          fontFamily: 'NotoSerifSC',
                          fontSize: 13.5,
                          fontWeight: FontWeight.w600,
                          color: CwColors.ink1,
                        ),
                      ),
                      const SizedBox(height: 2),
                      Text(where, style: const TextStyle(fontSize: 11, color: CwColors.ink3)),
                    ],
                  ),
                ),
                if (loading)
                  const SizedBox(
                    width: 16,
                    height: 16,
                    child: CircularProgressIndicator(strokeWidth: 2, color: CwColors.ink4),
                  )
                else
                  CwChip(label: chipLabel, variant: chipVariant, showDot: true),
              ],
            ),
            const SizedBox(height: 8),
            Padding(
              padding: const EdgeInsets.only(left: 52),
              child: loading
                  ? Container(
                      height: 10,
                      width: 80,
                      decoration: BoxDecoration(
                        color: CwColors.line,
                        borderRadius: BorderRadius.circular(4),
                      ),
                    )
                  : Text(
                      meta,
                      style: const TextStyle(
                        fontFamily: 'JetBrainsMono',
                        fontSize: 10,
                        color: CwColors.ink3,
                      ),
                    ),
            ),
            if (actionLabel != null && _isAuthenticated && !loading) ...[
              const SizedBox(height: 10),
              SizedBox(
                width: double.infinity,
                child: FilledButton(
                  onPressed: actionLoading ? null : onAction,
                  style: FilledButton.styleFrom(
                    backgroundColor: CwColors.accent,
                    foregroundColor: Colors.white,
                    minimumSize: const Size.fromHeight(40),
                    shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
                    textStyle: const TextStyle(fontSize: 13, fontWeight: FontWeight.w600),
                  ),
                  child: actionLoading
                      ? const SizedBox(width: 18, height: 18, child: CircularProgressIndicator(strokeWidth: 2, color: Colors.white))
                      : Text(actionLabel),
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}
