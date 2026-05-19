import 'dart:async';
import 'dart:convert';
import 'package:flutter/material.dart';
import '../theme/colors.dart';
import '../widgets/cw_orb.dart';
import '../widgets/top_toast.dart';
import '../l10n/strings.dart';
import '../main.dart';
import '../services/locator.dart';
import '../api/auth_api.dart';
import '../services/mpc_wallet_service.dart';
import '../services/mpc_session_manager.dart';
import '../platform/se_manager.dart';
import '../platform/sb_manager.dart';
import '../utils/device_id.dart';
import '../utils/secure_storage.dart';
import '../services/backup_shard_service.dart';

/// The onboarding stages of cowallet.
enum _Stage { hero, intro, email, emailOtp, creating, bio, pin, name, backup, ready, persona }

class OnboardingFlow extends StatefulWidget {
  const OnboardingFlow({super.key});

  @override
  State<OnboardingFlow> createState() => _OnboardingFlowState();
}

class _OnboardingFlowState extends State<OnboardingFlow> {
  _Stage _stage = _Stage.hero;

  // --- Intro PageView state ---
  final PageController _pageCtrl = PageController();
  int _guidePage = 0;

  // --- Creating stage state ---
  double _createProgress = 0;
  int _createChecksDone = 0; // 0..3
  Timer? _createTimer;
  bool _isResuming = false; // New: flag for resuming interrupted session

  // --- Bio stage state ---
  bool _bioAuthenticating = false;
  bool _bioDone = false;
  bool _bioError = false;

  // --- Email stage state ---
  final _emailCtrl = TextEditingController();
  String? _emailError;
  bool _emailSending = false;

  // --- Email OTP stage state ---
  String _otpInput = '';
  String? _otpError;
  bool _otpVerifying = false;

  // --- Name stage state ---
  final _nameCtrl = TextEditingController();

  bool _createError = false;


  // --- Persona stage state ---
  String? _selectedPersona;

  // --- PIN stage state ---
  String _pinInput = '';
  String? _pinFirst; // first entry, waiting for confirm
  bool _pinMismatch = false;
  bool _pinDone = false;

  // --- Backup stage state ---
  bool _backupSkipped = false;
  bool _backupSaving = false;
  bool _backupDone = false;

  // Keep track of navigation history for back button
  final List<_Stage> _history = [];


  @override
  void initState() {
    super.initState();
    _restoreStep();
  }

  Future<void> _restoreStep() async {
    final saved = await SecureStorage.get(SecureStorage.keyOnboardingStep);
    if (saved == null || saved.isEmpty) return;

    final stage = _Stage.values.where((s) => s.name == saved).firstOrNull;
    if (stage == null) return;

    // Don't restore to 'creating' — that needs a fresh DKG run
    if (stage == _Stage.creating) return;

    if (mounted) {
      setState(() => _stage = stage);
    }
  }

  @override
  void dispose() {
    _createTimer?.cancel();
    _emailCtrl.dispose();
    _nameCtrl.dispose();
    _pageCtrl.dispose();
    super.dispose();
  }


  void _goTo(_Stage next) {
    setState(() {
      _history.add(_stage);
      _stage = next;
    });
    SecureStorage.save(SecureStorage.keyOnboardingStep, next.name);
    if (next == _Stage.creating) _startCreating();
  }

  void _goBack() {
    if (_history.isNotEmpty) {
      setState(() {
        _stage = _history.removeLast();
      });
    }
  }

  // ---- Creating: MPC wallet generation + backend API integration ----
  void _startCreating() async {
    _createProgress = 0;
    _createChecksDone = 0;
    _createError = false;
    _isResuming = false;

    // Check for resumable session first
    final mpcService = Services.mpcWallet;
    final sessionManager = MpcSessionManager(mpcService);

    final canResume = await sessionManager.canResume();
    if (canResume && mounted) {
      setState(() => _isResuming = true);
      print('[OnboardingFlow] Found resumable session, attempting recovery...');
    }

    bool authDone = false;
    bool mpcSessionDone = false;
    bool mpcProtocolDone = false;
    bool walletDone = false;
    bool animDone = false;
    String? generatedAddress;

    void maybeAdvance() {
      if (!authDone || !mpcSessionDone || !mpcProtocolDone || !walletDone || !animDone || !mounted) return;
      if (generatedAddress != null) {
        CowalletApp.of(context).setWalletAddress(generatedAddress!);
        Future.delayed(const Duration(milliseconds: 400), () {
          if (mounted) _goTo(_Stage.backup);
        });
      }
    }

    // Step 1 → 2 → 3 顺序执行，确保 token 在 MPC 请求之前已保存
    () async {
      // Step 1: 设备注册/认证（含邮箱 + OTP 验证）
      try {
        final deviceId = await DeviceIdGenerator.getOrGenerate();
        final authResult = await AuthApi.register(
          deviceId: deviceId,
          email: _emailCtrl.text.trim(),
          otp: _otpInput,
        );
        if (!authResult.isSuccess) throw Exception(authResult.errorMessage);
        if (!mounted) return;
        setState(() => _createChecksDone = 1); // ✅ 设备验证通过
        authDone = true;
        maybeAdvance();
      } catch (e) {
        if (!mounted) return;
        _createTimer?.cancel();
        setState(() {
          _createError = true;
          _isResuming = false;
        });
        return; // 注册失败，终止后续步骤
      }

      // Step 2+3: 执行完整 DKG 协议（创建会话 + 多轮消息交换）
      try {
        if (mounted) {
          setState(() => _createChecksDone = 2); // ✅ MPC 会话建立
          mpcSessionDone = true;
        }

        // Use session manager for recovery support
        final walletInfo = await sessionManager.runDkgWithRecovery();
        generatedAddress = walletInfo.address;

        // Save pending backup shard to SecureStorage
        final backupShard = mpcService.lastBackupShard;
        if (backupShard != null && backupShard.isNotEmpty) {
          final base64Shard = base64Encode(backupShard);
          await SecureStorage.save(SecureStorage.keyPendingBackupShard, base64Shard);
          await SecureStorage.save(SecureStorage.keyPendingBackupCreatedAt, DateTime.now().toIso8601String());
          print('[OnboardingFlow] Saved pending backup shard to SecureStorage');
        }

        if (mounted) {
          setState(() {
            _createChecksDone = 3; // ✅ 密钥分片完成
            _isResuming = false;
          });
          mpcProtocolDone = true;
          walletDone = true;
          maybeAdvance();
        }
      } catch (e) {
        if (!mounted) return;
        _createTimer?.cancel();
        setState(() {
          _createError = true;
          _isResuming = false;
        });
        return;
      }
    }();

    // 动画时间线 (最小 2.5 秒保证用户体验)
    const tick = Duration(milliseconds: 50);
    int ticks = 0;
    _createTimer?.cancel();
    _createTimer = Timer.periodic(tick, (t) {
      if (!mounted) {
        t.cancel();
        return;
      }
      ticks++;
      setState(() {
        _createProgress = (ticks / 50).clamp(0.0, 1.0); // 50 ticks = 2.5s
        if (_createProgress >= 1.0) {
          t.cancel();
          animDone = true;
          maybeAdvance();
        }
      });
    });
  }

  // ---- Real biometric authentication ----
  Future<void> _startBioScan() async {
    // Immediately update UI before any async work
    setState(() {
      _bioAuthenticating = true;
      _bioError = false;
    });

    try {
      final available = await Services.biometrics.isAvailable();
      if (!mounted) return;

      if (!available) {
        await Services.biometrics.setEnabled(false);
        setState(() => _bioAuthenticating = false);
        _goTo(_Stage.name);
        return;
      }

      final hasEnrolled = await Services.biometrics.hasEnrolledBiometrics();
      if (!hasEnrolled) {
        await Services.biometrics.setEnabled(false);
        setState(() => _bioAuthenticating = false);
        _goTo(_Stage.name);
        return;
      }

      final authenticated = await Services.biometrics.authenticate(
        reason: 'Enable biometric protection for your wallet',
      );

      if (!mounted) return;
      setState(() => _bioAuthenticating = false);

      if (authenticated) {
        // Save biometric enabled status
        await Services.biometrics.setEnabled(true);

        // Initialize hardware-backed key store (required for secure signing)
        final seManager = SecureEnclaveManager();
        final sbManager = StrongBoxManager();
        if (await seManager.isAvailable()) {
          await seManager.initializeWallet('onboarding');
        } else if (await sbManager.isAvailable()) {
          await sbManager.initializeWallet('onboarding');
        } else {
          setState(() => _bioError = true);
          return;
        }

        setState(() => _bioDone = true);
        Future.delayed(const Duration(milliseconds: 600), () {
          if (mounted) _goTo(_Stage.name);
        });
      } else {
        setState(() => _bioError = true);
        // Show retry option with skip button
        await showDialog(
          context: context,
          builder: (ctx) => AlertDialog(
            title: const Text('Authentication Failed'),
            content: const Text('Biometric authentication helps secure your wallet. You can enable it later in Settings.'),
            actions: [
              TextButton(
                onPressed: () {
                  Navigator.pop(ctx);
                  _skipBio();
                },
                child: const Text('Skip for now'),
              ),
              TextButton(
                onPressed: () {
                  Navigator.pop(ctx);
                  setState(() => _bioError = false);
                },
                child: const Text('Retry'),
              ),
            ],
          ),
        );
      }
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _bioAuthenticating = false;
        _bioError = true;
      });
    }
  }

  void _skipBio() => _goTo(_Stage.pin);

  // ---- Name ----
  void _submitName() {
    final name = _nameCtrl.text.trim();
    if (name.isNotEmpty) {
      CowalletApp.of(context).setUserName(name);
    }
    _goTo(_Stage.ready);
  }

  // ---- Backup: store 3rd shard ----
  Future<void> _saveBackup({required bool useCloud}) async {
    setState(() => _backupSaving = true);
    try {
      final walletService = Services.wallet as MpcWalletService;
      var backupBytes = walletService.lastBackupShard;

      // If not in memory, try loading from pending storage
      if (backupBytes == null || backupBytes.isEmpty) {
        final pendingShard = await SecureStorage.get(SecureStorage.keyPendingBackupShard);
        if (pendingShard != null && pendingShard.isNotEmpty) {
          backupBytes = base64Decode(pendingShard);
          print('[OnboardingFlow] Loaded backup shard from pending storage');
        }
      }

      if (backupBytes == null || backupBytes.length != 32) {
        throw BackupException(BackupError.shardNotAvailable);
      }

      final result = await walletService.storeBackupShard(backupBytes, useCloud: useCloud);

      // Delete pending backup shard after successful backup
      await SecureStorage.delete(SecureStorage.keyPendingBackupShard);
      await SecureStorage.delete(SecureStorage.keyPendingBackupCreatedAt);
      print('[OnboardingFlow] Deleted pending backup shard after successful backup');

      if (!mounted) return;

      setState(() {
        _backupSaving = false;
        _backupDone = true;
      });

      final msg = result.method == BackupMethod.cloud
          ? S.backupSaved
          : S.backupFileSaved(result.filePath ?? '');
      showTopToast(context, msg, backgroundColor: CwColors.success);

      Future.delayed(const Duration(milliseconds: 600), () {
        if (mounted) _goTo(_Stage.bio);
      });
    } catch (e, st) {
      print('[OnboardingBackup] Error: $e');
      print('[OnboardingBackup] StackTrace: $st');
      if (!mounted) return;
      setState(() => _backupSaving = false);
      final errMsg = switch (e) {
        BackupException(error: BackupError.cloudUnavailable) => S.backupErrCloudUnavailable,
        BackupException(error: BackupError.cloudStoreFailed) => S.backupErrCloudStoreFailed,
        BackupException(error: BackupError.fileWriteFailed) => S.backupErrFileWriteFailed,
        BackupException(error: BackupError.shardNotAvailable) => S.backupErrShardNotAvailable,
        _ => S.backupErrCloudStoreFailed,
      };
      showTopToast(context, errMsg, backgroundColor: CwColors.danger);
    }
  }

  void _skipBackup() {
    setState(() => _backupSkipped = true);
    _goTo(_Stage.bio);
  }


  // ---- Persona ----
  void _pickPersona(String id) {
    setState(() => _selectedPersona = id);
    CowalletApp.of(context).setPersona(id);
    _finish();
  }

  void _skipPersona() => _finish();

  // ---- Finish ----
  Future<void> _finish() async {
    final appState = CowalletApp.of(context);
    appState.completeOnboarding();
    final addr = appState.walletAddress;

    // Clear persisted onboarding step
    await SecureStorage.delete(SecureStorage.keyOnboardingStep);

    // Persist onboarding metadata
    await SecureStorage.save('onboarding_completed_at', DateTime.now().toIso8601String());
    await SecureStorage.save('backup_status', _backupSkipped ? 'skipped' : (_backupDone ? 'saved' : 'pending'));
    await SecureStorage.save('mpc_address', addr);

    if (addr.isNotEmpty) {
      Services.balance.refresh(addr);
    }
    Navigator.pushReplacementNamed(context, '/');
  }

  // ======================= BUILD =======================

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: CwColors.bgPaper,
      body: SafeArea(
        child: AnimatedSwitcher(
          duration: const Duration(milliseconds: 300),
          transitionBuilder: (child, animation) {
            return FadeTransition(
              opacity: animation,
              child: child,
            );
          },
          layoutBuilder: (currentChild, previousChildren) {
            return Stack(
              alignment: Alignment.topCenter,
              fit: StackFit.expand,
              children: [
                ...previousChildren,
                if (currentChild != null) currentChild,
              ],
            );
          },
          child: _buildStage(),
        ),
      ),
    );
  }

  Widget _buildStage() {
    switch (_stage) {
      case _Stage.hero:
      case _Stage.intro:
        return _heroStage();
      case _Stage.email:
        return _emailStage();
      case _Stage.emailOtp:
        return _emailOtpStage();
      case _Stage.creating:
        return _creatingStage();
      case _Stage.bio:
        return _bioStage();
      case _Stage.pin:
        return _pinStage();
      case _Stage.name:
        return _nameStage();
      case _Stage.backup:
        return _backupStage();
      case _Stage.ready:
        return _readyStage();
      case _Stage.persona:
        return _personaStage();
    }
  }

  // ===================== SHARED WIDGETS =====================

  /// Top bar with optional back button and progress dots.
  Widget _topBar({bool showBack = false, int? step, int total = 3}) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 12),
      child: Row(
        children: [
          if (showBack)
            GestureDetector(
              onTap: _goBack,
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Icon(Icons.arrow_back_ios_new,
                      size: 16, color: CwColors.ink3),
                  const SizedBox(width: 4),
                  Text(S.back,
                      style: TextStyle(fontSize: 14, color: CwColors.ink3)),
                ],
              ),
            )
          else
            const SizedBox(width: 48),
          const Spacer(),
          if (step != null) _progressDots(step, total),
          const Spacer(),
          const SizedBox(width: 48),
        ],
      ),
    );
  }

  Widget _progressDots(int current, int total) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: List.generate(total, (i) {
        final isActive = i == current;
        final isDone = i < current;
        return Container(
          width: 8,
          height: 8,
          margin: const EdgeInsets.symmetric(horizontal: 4),
          decoration: BoxDecoration(
            shape: BoxShape.circle,
            color: (isActive || isDone) ? CwColors.accent : CwColors.line,
          ),
        );
      }),
    );
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
      style: Theme.of(context).textTheme.bodyLarge?.copyWith(
            color: CwColors.ink2,
          ),
      textAlign: TextAlign.center,
    );
  }

  Widget _primaryButton(String label, VoidCallback? onPressed) {
    return SizedBox(
      width: double.infinity,
      child: FilledButton(
        onPressed: onPressed,
        child: Text(label),
      ),
    );
  }

  Widget _secondaryLink(String label, VoidCallback onPressed) {
    return TextButton(
      onPressed: onPressed,
      child: Text(label, style: TextStyle(color: CwColors.ink3, fontSize: 14)),
    );
  }

  // ===================== STAGE 1+2: HERO + INTRO (PageView) =====================

  Widget _heroStage() {
    return Column(
      key: const ValueKey('hero'),
      children: [
        Expanded(
          child: PageView(
            controller: _pageCtrl,
            onPageChanged: (i) => setState(() => _guidePage = i),
            children: [
              _heroPage(),
              _introPageContent(),
            ],
          ),
        ),
        Padding(
          padding: const EdgeInsets.only(bottom: 32, left: 28, right: 28),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              // Page indicator dots
              Row(
                mainAxisAlignment: MainAxisAlignment.center,
                children: List.generate(2, (i) {
                  return AnimatedContainer(
                    duration: const Duration(milliseconds: 200),
                    width: i == _guidePage ? 20 : 8,
                    height: 8,
                    margin: const EdgeInsets.symmetric(horizontal: 4),
                    decoration: BoxDecoration(
                      borderRadius: BorderRadius.circular(4),
                      color: i == _guidePage ? CwColors.accent : CwColors.line,
                    ),
                  );
                }),
              ),
              const SizedBox(height: 24),
              // CTA button
              _primaryButton(
                _guidePage == 0 ? S.getStarted : S.introStart,
                () {
                  if (_guidePage == 0) {
                    _pageCtrl.animateToPage(1,
                        duration: const Duration(milliseconds: 300),
                        curve: Curves.easeInOut);
                  } else {
                    _goTo(_Stage.email);
                  }
                },
              ),
              const SizedBox(height: 12),
              TextButton(
                onPressed: () => Navigator.pushNamed(context, '/recovery'),
                child: Text(
                  S.recoverWallet,
                  style: TextStyle(color: CwColors.ink3, fontSize: 14),
                ),
              ),
              const SizedBox(height: 8),
              Text(
                S.heroLegal,
                style: Theme.of(context).textTheme.bodySmall?.copyWith(
                      color: CwColors.ink4,
                      fontSize: 11,
                    ),
                textAlign: TextAlign.center,
              ),
            ],
          ),
        ),
      ],
    );
  }

  Widget _heroPage() {
    return SingleChildScrollView(
      padding: const EdgeInsets.symmetric(horizontal: 28),
      child: Column(
        children: [
          const SizedBox(height: 48),
          const CwOrb(size: 140, breathing: true),
          const SizedBox(height: 28),
          Text(
            S.heroKicker,
            style: Theme.of(context).textTheme.labelLarge?.copyWith(
                  color: CwColors.ink3,
                  letterSpacing: 1.2,
                ),
            textAlign: TextAlign.center,
          ),
          const SizedBox(height: 12),
          RichText(
            textAlign: TextAlign.center,
            text: TextSpan(
              style: Theme.of(context).textTheme.displayLarge,
              children: [
                TextSpan(text: S.heroH1a),
                if (S.heroH1b.isNotEmpty)
                  TextSpan(
                    text: ' ${S.heroH1b} ',
                    style: Theme.of(context).textTheme.displayLarge,
                  ),
                TextSpan(
                  text: S.heroH1em,
                  style: Theme.of(context).textTheme.displayLarge?.copyWith(
                        fontStyle: FontStyle.italic,
                        color: CwColors.accent,
                      ),
                ),
              ],
            ),
          ),
          const SizedBox(height: 16),
          Text(
            S.heroExplain,
            style: Theme.of(context).textTheme.bodyLarge?.copyWith(
                  color: CwColors.ink2,
                ),
            textAlign: TextAlign.center,
          ),
          const SizedBox(height: 32),
          _featureRow(Icons.touch_app_outlined, S.heroFeat1h, S.heroFeat1s),
          const SizedBox(height: 16),
          _featureRow(Icons.public, S.heroFeat2h, S.heroFeat2s),
          const SizedBox(height: 16),
          _featureRow(Icons.auto_awesome, S.heroFeat3h, S.heroFeat3s),
          const SizedBox(height: 24),
        ],
      ),
    );
  }

  Widget _introPageContent() {
    return SingleChildScrollView(
      padding: const EdgeInsets.symmetric(horizontal: 28),
      child: Column(
        children: [
          const SizedBox(height: 48),
          Icon(Icons.lock_outline, size: 64, color: CwColors.accent),
          const SizedBox(height: 24),
          _heading(S.introH1),
          const SizedBox(height: 12),
          _subtitle(S.introSub),
          const SizedBox(height: 32),
          _featureRow(Icons.call_split, S.introBullet1h, S.introBullet1s),
          const SizedBox(height: 16),
          _featureRow(Icons.verified_user_outlined, S.introBullet2h, S.introBullet2s),
          const SizedBox(height: 16),
          _featureRow(Icons.hide_source_outlined, S.introBullet3h, S.introBullet3s),
          const SizedBox(height: 24),
        ],
      ),
    );
  }

  Widget _featureRow(IconData icon, String title, String sub) {
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Container(
          width: 40,
          height: 40,
          decoration: BoxDecoration(
            color: CwColors.accentSoft,
            borderRadius: BorderRadius.circular(10),
          ),
          child: Icon(icon, size: 20, color: CwColors.accent),
        ),
        const SizedBox(width: 14),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(title,
                  style: Theme.of(context)
                      .textTheme
                      .titleMedium
                      ?.copyWith(color: CwColors.ink1)),
              const SizedBox(height: 2),
              Text(sub,
                  style: Theme.of(context)
                      .textTheme
                      .bodySmall
                      ?.copyWith(color: CwColors.ink3)),
            ],
          ),
        ),
      ],
    );
  }

  // ===================== STAGE 2.5: EMAIL =====================

  Future<void> _submitEmail() async {
    final email = _emailCtrl.text.trim();
    if (email.isEmpty || !email.contains('@') || !email.contains('.')) {
      setState(() => _emailError = S.invalidEmail);
      return;
    }
    setState(() {
      _emailError = null;
      _emailSending = true;
    });

    try {
      final result = await AuthApi.sendEmailOtp(email: email);
      if (!mounted) return;
      if (result.isSuccess) {
        setState(() => _emailSending = false);
        _goTo(_Stage.emailOtp);
      } else {
        setState(() {
          _emailSending = false;
          _emailError = result.errorMessage ?? S.emailSendFailed;
        });
      }
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _emailSending = false;
        _emailError = S.emailSendFailed;
      });
    }
  }

  // ===================== STAGE 2.6: EMAIL OTP =====================

  void _onOtpDigit(String digit) {
    if (_otpInput.length >= 6) return;
    setState(() {
      _otpInput += digit;
      _otpError = null;
    });
    if (_otpInput.length == 6) {
      _verifyEmailOtp();
    }
  }

  void _onOtpBackspace() {
    if (_otpInput.isEmpty) return;
    setState(() {
      _otpInput = _otpInput.substring(0, _otpInput.length - 1);
      _otpError = null;
    });
  }

  Future<void> _verifyEmailOtp() async {
    // OTP will be verified during register — just proceed to creating
    setState(() => _otpVerifying = false);
    _goTo(_Stage.creating);
  }

  Future<void> _resendOtp() async {
    setState(() => _otpInput = '');
    final result = await AuthApi.sendEmailOtp(email: _emailCtrl.text.trim());
    if (!mounted) return;
    if (!result.isSuccess) {
      setState(() => _otpError = S.emailSendFailed);
    }
  }

  Widget _emailOtpStage() {
    return SingleChildScrollView(
      key: const ValueKey('emailOtp'),
      child: Column(
        children: [
          _topBar(showBack: true, step: 0, total: 3),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 28),
            child: Column(
              children: [
                const SizedBox(height: 40),
                Icon(Icons.mark_email_read_outlined, size: 56, color: CwColors.accent),
                const SizedBox(height: 24),
                _heading(S.otpH1),
                const SizedBox(height: 8),
                _subtitle(S.otpSub(_emailCtrl.text.trim())),
                const SizedBox(height: 32),
                // OTP dots
                Row(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: List.generate(6, (i) {
                    final filled = i < _otpInput.length;
                    return Container(
                      width: 16,
                      height: 16,
                      margin: const EdgeInsets.symmetric(horizontal: 8),
                      decoration: BoxDecoration(
                        shape: BoxShape.circle,
                        color: filled ? CwColors.accent : Colors.transparent,
                        border: Border.all(
                          color: filled ? CwColors.accent : CwColors.ink4,
                          width: 2,
                        ),
                      ),
                    );
                  }),
                ),
                if (_otpError != null) ...[
                  const SizedBox(height: 12),
                  Text(_otpError!, style: TextStyle(fontSize: 13, color: CwColors.danger)),
                ],
                const SizedBox(height: 32),
                if (_otpVerifying)
                  const CircularProgressIndicator()
                else
                  _buildOtpNumPad(),
                const SizedBox(height: 24),
                _secondaryLink(S.otpResend, _resendOtp),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildOtpNumPad() {
    return Column(
      children: [
        for (final row in [['1','2','3'], ['4','5','6'], ['7','8','9'], ['','0','⌫']])
          Padding(
            padding: const EdgeInsets.only(bottom: 12),
            child: Row(
              mainAxisAlignment: MainAxisAlignment.center,
              children: row.map((key) {
                if (key.isEmpty) return const SizedBox(width: 72, height: 56);
                return GestureDetector(
                  onTap: () {
                    if (key == '⌫') {
                      _onOtpBackspace();
                    } else {
                      _onOtpDigit(key);
                    }
                  },
                  child: Container(
                    width: 72,
                    height: 56,
                    margin: const EdgeInsets.symmetric(horizontal: 8),
                    decoration: BoxDecoration(
                      color: CwColors.bgCard,
                      borderRadius: BorderRadius.circular(14),
                      border: Border.all(color: CwColors.line),
                    ),
                    alignment: Alignment.center,
                    child: Text(
                      key,
                      style: TextStyle(
                        fontSize: key == '⌫' ? 20 : 24,
                        fontWeight: FontWeight.w500,
                        color: CwColors.ink1,
                      ),
                    ),
                  ),
                );
              }).toList(),
            ),
          ),
      ],
    );
  }

  Widget _emailStage() {
    return SingleChildScrollView(
      key: const ValueKey('email'),
      child: Column(
        children: [
          _topBar(showBack: true, step: 0, total: 3),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 28),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                const SizedBox(height: 40),
                Center(
                  child: Icon(Icons.email_outlined, size: 56, color: CwColors.accent),
                ),
                const SizedBox(height: 24),
                Center(child: _heading(S.emailH1)),
                const SizedBox(height: 8),
                Center(child: _subtitle(S.emailSub)),
                const SizedBox(height: 32),
                Container(
                  decoration: BoxDecoration(
                    color: CwColors.bgCard,
                    borderRadius: BorderRadius.circular(14),
                    border: Border.all(
                      color: _emailError != null ? CwColors.danger : CwColors.line,
                    ),
                  ),
                  child: TextField(
                    controller: _emailCtrl,
                    keyboardType: TextInputType.emailAddress,
                    autocorrect: false,
                    style: const TextStyle(fontSize: 16, color: CwColors.ink1),
                    decoration: InputDecoration(
                      hintText: 'your@email.com',
                      hintStyle: TextStyle(fontSize: 16, color: CwColors.ink4),
                      contentPadding: const EdgeInsets.symmetric(
                          horizontal: 16, vertical: 16),
                      border: InputBorder.none,
                      prefixIcon: Icon(Icons.mail_outline, color: CwColors.ink3),
                    ),
                    onSubmitted: (_) => _submitEmail(),
                    onChanged: (_) {
                      if (_emailError != null) setState(() => _emailError = null);
                    },
                  ),
                ),
                if (_emailError != null) ...[
                  const SizedBox(height: 8),
                  Text(
                    _emailError!,
                    style: TextStyle(fontSize: 13, color: CwColors.danger),
                  ),
                ],
                const SizedBox(height: 12),
                Text(
                  S.emailHint,
                  style: Theme.of(context)
                      .textTheme
                      .bodySmall
                      ?.copyWith(color: CwColors.ink4),
                ),
                const SizedBox(height: 32),
                _primaryButton(
                  _emailSending ? '...' : S.continueBtn,
                  _emailSending ? null : _submitEmail,
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  // ===================== STAGE 3: CREATING =====================

  Widget _creatingStage() {
    return SingleChildScrollView(
      key: const ValueKey('creating'),
      child: Column(
        children: [
          _topBar(step: 1, total: 3),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 28),
            child: Column(
              children: [
                const SizedBox(height: 24),
                const CwOrb(size: 120, thinking: true),
                const SizedBox(height: 28),
                _heading(_isResuming ? 'Resuming...' : S.creatingH1),
                const SizedBox(height: 8),
                _subtitle(_isResuming ? 'Recovering your wallet session' : S.creatingSub),
                const SizedBox(height: 32),
                // Progress bar
                ClipRRect(
                  borderRadius: BorderRadius.circular(6),
                  child: LinearProgressIndicator(
                    value: _createProgress,
                    minHeight: 8,
                    backgroundColor: CwColors.line,
                    valueColor:
                        const AlwaysStoppedAnimation<Color>(CwColors.accent),
                  ),
                ),
                const SizedBox(height: 8),
                Align(
                  alignment: Alignment.centerRight,
                  child: Text(
                    '${(_createProgress * 100).clamp(0, 100).toInt()}%',
                    style: Theme.of(context)
                        .textTheme
                        .labelMedium
                        ?.copyWith(color: CwColors.ink3),
                  ),
                ),
                const SizedBox(height: 24),
                // 3 check-lines
                _checkLine(S.cl1, _createChecksDone >= 1),
                const SizedBox(height: 12),
                _checkLine(S.cl2, _createChecksDone >= 2),
                const SizedBox(height: 12),
                _checkLine(S.cl3, _createChecksDone >= 3),
                if (_createError) ...[
                  const SizedBox(height: 24),
                  Container(
                    width: double.infinity,
                    padding: const EdgeInsets.all(14),
                    decoration: BoxDecoration(
                      color: CwColors.warnSoft,
                      borderRadius: BorderRadius.circular(12),
                      border: Border.all(
                          color: CwColors.warn.withValues(alpha: 0.3)),
                    ),
                    child: Row(
                      children: [
                        Icon(Icons.error_outline,
                            size: 20, color: CwColors.warn),
                        const SizedBox(width: 10),
                        Expanded(
                          child: Text(S.createError,
                              style: TextStyle(
                                  fontSize: 13, color: CwColors.ink2)),
                        ),
                      ],
                    ),
                  ),
                  const SizedBox(height: 16),
                  _primaryButton(S.retry, _startCreating),
                ],
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
              ? Icon(Icons.check_circle, key: ValueKey('$text-done'),
                  size: 20, color: CwColors.success)
              : SizedBox(
                  key: ValueKey('$text-wait'),
                  width: 20,
                  height: 20,
                  child: CircularProgressIndicator(
                    strokeWidth: 2,
                    color: CwColors.ink4,
                  ),
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

  // ===================== STAGE 4: IMPORTING =====================


  // ===================== STAGE 5: BIO =====================

  Widget _bioStage() {
    return SingleChildScrollView(
      key: const ValueKey('bio'),
      child: Column(
        children: [
          _topBar(step: 2, total: 3),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 28),
            child: Column(
              children: [
                const SizedBox(height: 40),
                Icon(
                  _bioDone ? Icons.check_circle : Icons.fingerprint,
                  size: 64,
                  color: _bioDone ? CwColors.success : CwColors.accent,
                ),
                const SizedBox(height: 32),
                _heading(_bioDone ? S.bioDone : S.bioH1),
                const SizedBox(height: 8),
                _subtitle(S.bioSub),
                const SizedBox(height: 40),
                if (!_bioDone && !_bioAuthenticating) ...[
                  _primaryButton(S.bioActivate, _startBioScan),
                  const SizedBox(height: 12),
                  _secondaryLink(S.bioSkip, _skipBio),
                ],
                if (_bioAuthenticating) ...[
                  const SizedBox(
                    width: 28,
                    height: 28,
                    child: CircularProgressIndicator(strokeWidth: 2.5),
                  ),
                  const SizedBox(height: 12),
                  Text(
                    S.bioVerifying,
                    style: TextStyle(fontSize: 14, color: CwColors.ink3),
                  ),
                ],
              ],
            ),
          ),
        ],
      ),
    );
  }

  // ===================== STAGE 5b: PIN =====================

  void _onPinDigit(String digit) {
    if (_pinInput.length >= 6) return;
    setState(() {
      _pinInput += digit;
      _pinMismatch = false;
    });
    if (_pinInput.length == 6) {
      _onPinComplete(_pinInput);
    }
  }

  void _onPinBackspace() {
    if (_pinInput.isEmpty) return;
    setState(() {
      _pinInput = _pinInput.substring(0, _pinInput.length - 1);
      _pinMismatch = false;
    });
  }

  Future<void> _onPinComplete(String pin) async {
    if (_pinFirst == null) {
      setState(() {
        _pinFirst = pin;
        _pinInput = '';
      });
    } else {
      if (pin == _pinFirst) {
        await SecureStorage.save('wallet_pin', pin);
        setState(() => _pinDone = true);
        Future.delayed(const Duration(milliseconds: 600), () {
          if (mounted) _goTo(_Stage.name);
        });
      } else {
        setState(() {
          _pinMismatch = true;
          _pinInput = '';
          _pinFirst = null;
        });
      }
    }
  }

  Widget _pinStage() {
    final isConfirm = _pinFirst != null && !_pinDone;
    return SingleChildScrollView(
      key: const ValueKey('pin'),
      child: Column(
        children: [
          _topBar(step: 2, total: 3),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 28),
            child: Column(
              children: [
                const SizedBox(height: 40),
                Icon(
                  _pinDone ? Icons.check_circle : Icons.lock_outline,
                  size: 56,
                  color: _pinDone ? CwColors.success : CwColors.accent,
                ),
                const SizedBox(height: 32),
                _heading(_pinDone ? S.pinDone : (isConfirm ? S.pinConfirmH1 : S.pinH1)),
                const SizedBox(height: 8),
                _subtitle(isConfirm ? S.pinConfirmSub : S.pinSub),
                const SizedBox(height: 32),
                if (_pinMismatch)
                  Padding(
                    padding: const EdgeInsets.only(bottom: 16),
                    child: Text(
                      S.pinMismatch,
                      style: TextStyle(color: CwColors.danger, fontSize: 14),
                    ),
                  ),
                if (!_pinDone) ...[
                  Row(
                    mainAxisAlignment: MainAxisAlignment.center,
                    children: List.generate(6, (i) {
                      final filled = i < _pinInput.length;
                      return Container(
                        width: 16,
                        height: 16,
                        margin: const EdgeInsets.symmetric(horizontal: 8),
                        decoration: BoxDecoration(
                          shape: BoxShape.circle,
                          color: filled ? CwColors.accent : Colors.transparent,
                          border: Border.all(
                            color: filled ? CwColors.accent : CwColors.ink4,
                            width: 2,
                          ),
                        ),
                      );
                    }),
                  ),
                  const SizedBox(height: 40),
                  _buildNumPad(),
                ],
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildNumPad() {
    return Column(
      children: [
        for (final row in [['1','2','3'], ['4','5','6'], ['7','8','9'], ['','0','⌫']])
          Padding(
            padding: const EdgeInsets.only(bottom: 12),
            child: Row(
              mainAxisAlignment: MainAxisAlignment.center,
              children: row.map((key) {
                if (key.isEmpty) return const SizedBox(width: 72, height: 56);
                return GestureDetector(
                  onTap: () {
                    if (key == '⌫') {
                      _onPinBackspace();
                    } else {
                      _onPinDigit(key);
                    }
                  },
                  child: Container(
                    width: 72,
                    height: 56,
                    margin: const EdgeInsets.symmetric(horizontal: 8),
                    decoration: BoxDecoration(
                      color: CwColors.bgCard,
                      borderRadius: BorderRadius.circular(14),
                      border: Border.all(color: CwColors.line),
                    ),
                    alignment: Alignment.center,
                    child: Text(
                      key,
                      style: TextStyle(
                        fontSize: key == '⌫' ? 20 : 24,
                        fontWeight: FontWeight.w500,
                        color: CwColors.ink1,
                      ),
                    ),
                  ),
                );
              }).toList(),
            ),
          ),
      ],
    );
  }

  // ===================== STAGE 6: NAME =====================

  Widget _nameStage() {
    return SingleChildScrollView(
      key: const ValueKey('name'),
      child: Column(
        children: [
          _topBar(step: 2, total: 3),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 28),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                const SizedBox(height: 24),
                Center(child: _heading(S.nameH1)),
                const SizedBox(height: 28),
                // Text input
                Container(
                  decoration: BoxDecoration(
                    color: CwColors.bgCard,
                    borderRadius: BorderRadius.circular(14),
                    border: Border.all(color: CwColors.line),
                  ),
                  child: TextField(
                    controller: _nameCtrl,
                    textCapitalization: TextCapitalization.words,
                    style: const TextStyle(
                      fontFamily: 'NotoSerifSC',
                      fontSize: 20,
                      fontWeight: FontWeight.w500,
                      color: CwColors.ink1,
                    ),
                    textAlign: TextAlign.center,
                    decoration: InputDecoration(
                      hintText: S.namePlaceholder,
                      hintStyle: TextStyle(
                        fontFamily: 'NotoSerifSC',
                        fontSize: 20,
                        fontWeight: FontWeight.w400,
                        color: CwColors.ink4,
                      ),
                      contentPadding: const EdgeInsets.symmetric(
                          horizontal: 16, vertical: 18),
                      border: InputBorder.none,
                    ),
                    onSubmitted: (_) => _submitName(),
                  ),
                ),
                const SizedBox(height: 10),
                // Hint
                Center(
                  child: Text(
                    S.nameHint,
                    style: Theme.of(context)
                        .textTheme
                        .bodySmall
                        ?.copyWith(color: CwColors.ink4),
                  ),
                ),
                const SizedBox(height: 32),
                _primaryButton(S.continueBtn, _submitName),
              ],
            ),
          ),
        ],
      ),
    );
  }

  // ===================== STAGE 7: BACKUP (store 3rd shard) =====================

  Widget _backupStage() {
    return SingleChildScrollView(
      key: const ValueKey('backup'),
      child: Column(
        children: [
          _topBar(step: 1, total: 3),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 28),
            child: Column(
              children: [
                const SizedBox(height: 24),
                Icon(Icons.shield_outlined, size: 64, color: CwColors.accent),
                const SizedBox(height: 24),
                _heading(S.backupH1),
                const SizedBox(height: 8),
                _subtitle(S.backupSub),
                const SizedBox(height: 32),
                if (_backupDone) ...[
                  Container(
                    width: double.infinity,
                    padding: const EdgeInsets.all(16),
                    decoration: BoxDecoration(
                      color: CwColors.success.withValues(alpha: 0.1),
                      borderRadius: BorderRadius.circular(14),
                      border: Border.all(color: CwColors.success.withValues(alpha: 0.3)),
                    ),
                    child: Row(
                      mainAxisAlignment: MainAxisAlignment.center,
                      children: [
                        Icon(Icons.check_circle, size: 20, color: CwColors.success),
                        const SizedBox(width: 10),
                        Text(
                          S.backupSaved,
                          style: TextStyle(fontSize: 15, color: CwColors.success, fontWeight: FontWeight.w600),
                        ),
                      ],
                    ),
                  ),
                ] else if (_backupSaving) ...[
                  const CircularProgressIndicator(),
                  const SizedBox(height: 16),
                  Text(S.backupSaving, style: TextStyle(color: CwColors.ink3)),
                ] else ...[
                  _backupOptionCard(
                    icon: Icons.cloud_upload_outlined,
                    title: S.backupCloudTitle,
                    desc: S.backupCloudDesc,
                    onTap: () => _saveBackup(useCloud: true),
                  ),
                  const SizedBox(height: 12),
                  _backupOptionCard(
                    icon: Icons.save_alt_outlined,
                    title: S.backupFileTitle,
                    desc: S.backupFileDesc,
                    onTap: () => _saveBackup(useCloud: false),
                  ),
                  const SizedBox(height: 24),
                  Center(
                    child: _secondaryLink(S.backupSkip, _skipBackup),
                  ),
                ],
                const SizedBox(height: 24),
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
                  Text(title,
                      style: TextStyle(
                          fontSize: 15,
                          fontWeight: FontWeight.w600,
                          color: CwColors.ink1)),
                  const SizedBox(height: 2),
                  Text(desc,
                      style: TextStyle(fontSize: 13, color: CwColors.ink3)),
                ],
              ),
            ),
            Icon(Icons.chevron_right, size: 20, color: CwColors.ink4),
          ],
        ),
      ),
    );
  }

  // ===================== STAGE 9: READY =====================

  Widget _readyStage() {
    final name = CowalletApp.of(context).userName;
    final h1 = name.isNotEmpty ? S.readyH1Named(name) : S.readyH1;

    return SingleChildScrollView(
      key: const ValueKey('ready'),
      padding: const EdgeInsets.symmetric(horizontal: 28),
      child: Column(
        children: [
          const SizedBox(height: 48),
          // CwOrb with checkmark badge
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
          _heading(h1),
          const SizedBox(height: 8),
          _subtitle(S.readySub),
          const SizedBox(height: 32),
          // "What you can do next" label
          Align(
            alignment: Alignment.centerLeft,
            child: Text(
              S.readyWhat,
              style: Theme.of(context)
                  .textTheme
                  .labelLarge
                  ?.copyWith(color: CwColors.ink3),
            ),
          ),
          const SizedBox(height: 16),
          // 3 numbered next-steps
          _numberedStep(1, S.ready1h, S.ready1s),
          const SizedBox(height: 12),
          _numberedStep(2, S.ready2h, S.ready2s),
          const SizedBox(height: 12),
          _numberedStep(3, S.ready3h, S.ready3s),
          const SizedBox(height: 36),
          _primaryButton(S.readyGo, () => _goTo(_Stage.persona)),
          const SizedBox(height: 24),
        ],
      ),
    );
  }

  Widget _numberedStep(int n, String title, String sub) {
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Container(
          width: 28,
          height: 28,
          decoration: BoxDecoration(
            color: CwColors.accentSoft,
            shape: BoxShape.circle,
          ),
          alignment: Alignment.center,
          child: Text(
            '$n',
            style: TextStyle(
              fontFamily: 'JetBrainsMono',
              fontSize: 13,
              fontWeight: FontWeight.w600,
              color: CwColors.accent,
            ),
          ),
        ),
        const SizedBox(width: 14),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(title,
                  style: Theme.of(context)
                      .textTheme
                      .titleMedium
                      ?.copyWith(color: CwColors.ink1)),
              const SizedBox(height: 2),
              Text(sub,
                  style: Theme.of(context)
                      .textTheme
                      .bodySmall
                      ?.copyWith(color: CwColors.ink3)),
            ],
          ),
        ),
      ],
    );
  }

  // ===================== STAGE 8: PERSONA =====================

  Widget _personaStage() {
    return SingleChildScrollView(
      key: const ValueKey('persona'),
      child: Column(
        children: [
          const SizedBox(height: 48),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 28),
            child: Column(
              children: [
                _heading(S.personaH1),
                const SizedBox(height: 8),
                _subtitle(S.personaSub),
                const SizedBox(height: 28),
                _personaCard(
                  id: 'daily',
                  icon: Icons.wb_sunny_outlined,
                  title: S.personaDaily,
                  desc: S.personaDailyDesc,
                  tag: S.personaDailyTag,
                ),
                const SizedBox(height: 12),
                _personaCard(
                  id: 'trader',
                  icon: Icons.candlestick_chart,
                  title: S.personaTrader,
                  desc: S.personaTraderDesc,
                ),
                const SizedBox(height: 12),
                _personaCard(
                  id: 'family',
                  icon: Icons.people_outline,
                  title: S.personaFamily,
                  desc: S.personaFamilyDesc,
                  tag: S.personaFamilyTag,
                ),
                const SizedBox(height: 12),
                _personaCard(
                  id: 'builder',
                  icon: Icons.terminal,
                  title: S.personaBuilder,
                  desc: S.personaBuilderDesc,
                ),
                const SizedBox(height: 24),
                _secondaryLink(S.personaSkip, _skipPersona),
                const SizedBox(height: 24),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _personaCard({
    required String id,
    required IconData icon,
    required String title,
    required String desc,
    String? tag,
  }) {
    final selected = _selectedPersona == id;
    return GestureDetector(
      onTap: () => _pickPersona(id),
      child: AnimatedContainer(
        duration: const Duration(milliseconds: 200),
        width: double.infinity,
        padding: const EdgeInsets.all(16),
        decoration: BoxDecoration(
          color: selected ? CwColors.accentSoft : CwColors.bgCard,
          borderRadius: BorderRadius.circular(16),
          border: Border.all(
            color: selected ? CwColors.accent : CwColors.line,
            width: selected ? 2 : 1,
          ),
        ),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Container(
              width: 40,
              height: 40,
              decoration: BoxDecoration(
                color: selected
                    ? CwColors.accent.withValues(alpha: 0.15)
                    : CwColors.accentSoft,
                borderRadius: BorderRadius.circular(10),
              ),
              child: Icon(icon, size: 20, color: CwColors.accent),
            ),
            const SizedBox(width: 14),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Row(
                    children: [
                      Expanded(
                        child: Text(title,
                            style: Theme.of(context)
                                .textTheme
                                .titleMedium
                                ?.copyWith(
                                    color: CwColors.ink1,
                                    fontWeight: FontWeight.w600)),
                      ),
                      if (tag != null)
                        Container(
                          padding: const EdgeInsets.symmetric(
                              horizontal: 8, vertical: 2),
                          decoration: BoxDecoration(
                            color: CwColors.accentSoft,
                            borderRadius: BorderRadius.circular(6),
                          ),
                          child: Text(tag,
                              style: TextStyle(
                                  fontSize: 11,
                                  color: CwColors.accent,
                                  fontWeight: FontWeight.w600)),
                        ),
                    ],
                  ),
                  const SizedBox(height: 4),
                  Text(desc,
                      style: Theme.of(context)
                          .textTheme
                          .bodyMedium
                          ?.copyWith(color: CwColors.ink3)),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}
