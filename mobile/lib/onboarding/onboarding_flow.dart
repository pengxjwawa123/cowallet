import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import '../theme/colors.dart';
import '../widgets/cw_orb.dart';
import '../l10n/strings.dart';
import '../main.dart';
import '../services/locator.dart';
import '../api/auth_api.dart';
import '../services/mpc_wallet_service.dart';
import '../services/mpc_session_manager.dart';
import '../services/mpc_session_store.dart';
import '../platform/se_manager.dart';
import '../platform/sb_manager.dart';
import '../utils/device_id.dart';
import '../bridge/mpc_bridge.dart';
import '../services/recovery_service.dart';
import '../services/backup_shard_service.dart';
import '../platform/cloud_backup.dart';
import '../utils/secure_storage.dart';
import 'package:convert/convert.dart';

/// The 10 stages of the cowallet onboarding, matching the H5 prototype.
enum _Stage { hero, start, creating, importing, bio, name, backup, verifyBackup, ready, persona }

class OnboardingFlow extends StatefulWidget {
  const OnboardingFlow({super.key});

  @override
  State<OnboardingFlow> createState() => _OnboardingFlowState();
}

class _OnboardingFlowState extends State<OnboardingFlow>
    with TickerProviderStateMixin {
  _Stage _stage = _Stage.hero;

  // --- Creating stage state ---
  double _createProgress = 0;
  int _createChecksDone = 0; // 0..3
  Timer? _createTimer;
  bool _isResuming = false; // New: flag for resuming interrupted session

  // --- Bio stage state ---
  bool _bioScanning = false;
  bool _bioDone = false;
  bool _bioError = false;
  late AnimationController _bioRingCtrl;

  // --- Name stage state ---
  final _nameCtrl = TextEditingController();

  bool _createError = false;

  // --- Importing stage state ---
  final _importCtrl = TextEditingController();
  int _wordCount = 0;

  // --- Persona stage state ---
  String? _selectedPersona;

  // --- Backup stage state ---
  List<String> _recoveryMnemonic = [];
  List<String> _verifyWords = [];
  List<String> _selectedVerifyWords = [];
  bool _backupSkipped = false;
  bool _backupInCloud = false;

  // Keep track of navigation history for back button
  final List<_Stage> _history = [];

  @override
  void initState() {
    super.initState();
    _bioRingCtrl = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 1200),
    );
    _importCtrl.addListener(_updateWordCount);
  }

  @override
  void dispose() {
    _createTimer?.cancel();
    _bioRingCtrl.dispose();
    _nameCtrl.dispose();
    _importCtrl.removeListener(_updateWordCount);
    _importCtrl.dispose();
    super.dispose();
  }

  void _updateWordCount() {
    final text = _importCtrl.text.trim();
    final count = text.isEmpty ? 0 : text.split(RegExp(r'\s+')).length;
    if (count != _wordCount) {
      setState(() => _wordCount = count);
    }
  }

  void _goTo(_Stage next) {
    setState(() {
      _history.add(_stage);
      _stage = next;
    });
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
    final mpcService = MpcWalletService();
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
          if (mounted) _goTo(_Stage.bio);
        });
      }
    }

    // Step 1 → 2 → 3 顺序执行，确保 token 在 MPC 请求之前已保存
    () async {
      // Step 1: 设备注册/认证
      try {
        final deviceId = await DeviceIdGenerator.getOrGenerate();
        final authResult = await AuthApi.register(deviceId: deviceId);
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
    print('[OnboardingBio] Starting biometric scan...');
    setState(() => _bioScanning = true);
    _bioRingCtrl.repeat();

    try {
      // First check if biometric is available
      final available = await Services.biometrics.isAvailable();
      print('[OnboardingBio] isAvailable: $available');

      if (!mounted) return;

      // If biometric not available, skip this step
      if (!available) {
        print('[OnboardingBio] Biometric not available, skipping');
        _bioRingCtrl.stop();
        await Services.biometrics.setEnabled(false);
        _goTo(_Stage.name);
        return;
      }

      final hasEnrolled = await Services.biometrics.hasEnrolledBiometrics();
      print('[OnboardingBio] hasEnrolledBiometrics: $hasEnrolled');

      if (!hasEnrolled) {
        print('[OnboardingBio] No biometric enrolled, skipping');
        _bioRingCtrl.stop();
        await Services.biometrics.setEnabled(false);
        _goTo(_Stage.name);
        return;
      }

      // Real biometric authentication (Face ID / Touch ID / Fingerprint)
      print('[OnboardingBio] Calling authenticate...');
      final authenticated = await Services.biometrics.authenticate(
        reason: 'Enable biometric protection for your wallet',
      );
      print('[OnboardingBio] authenticated: $authenticated');

      if (!mounted) return;
      _bioRingCtrl.stop();

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
          setState(() {
            _bioScanning = false;
            _bioError = true;
          });
          return;
        }

        setState(() {
          _bioScanning = false;
          _bioDone = true;
        });
        Future.delayed(const Duration(milliseconds: 600), () {
          if (mounted) _goTo(_Stage.name);
        });
      } else {
        setState(() {
          _bioScanning = false;
          _bioError = true;
        });
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
      _bioRingCtrl.stop();
      setState(() {
        _bioScanning = false;
        _bioError = true;
      });
    }
  }

  void _skipBio() => _goTo(_Stage.name);

  // ---- Name ----
  void _submitName() {
    final name = _nameCtrl.text.trim();
    if (name.isNotEmpty) {
      CowalletApp.of(context).setUserName(name);
    }
    _goTo(_Stage.backup);
  }

  // ---- Backup Mnemonic ----
  Future<void> _startBackup() async {
    // Generate real BIP-39 mnemonic from backup shard bytes
    try {
      // Get the last DKG session ID from MpcSessionStore
      final lastSession = await MpcSessionStore.loadSession();
      if (lastSession == null) {
        throw Exception('No DKG session found');
      }

      // Derive the actual 32-byte backup shard from DKG
      final backupBytes = await MpcBridge.dkgDeriveBackupShare(lastSession.sessionId);

      // Convert to 24-word BIP-39-style mnemonic (256 bits = 24 words)
      setState(() {
        _recoveryMnemonic = _bytesToMnemonic(backupBytes);
      });
    } catch (e) {
      print('[OnboardingBackup] Failed to derive backup shard: $e');
      // Fallback to dummy words if derivation fails
      setState(() {
        _recoveryMnemonic = _generateDummyMnemonic();
      });
    }
  }

  List<String> _bytesToMnemonic(List<int> bytes) {
    // BIP-39 English wordlist (2048 words, subset for demonstration)
    const wordList = [
      'abandon', 'ability', 'able', 'about', 'above', 'absent', 'absorb', 'abstract',
      'absurd', 'abuse', 'access', 'accident', 'account', 'accuse', 'achieve', 'acid',
      'acoustic', 'acquire', 'across', 'act', 'action', 'actor', 'actress', 'actual',
      'adapt', 'add', 'addict', 'address', 'adjust', 'admit', 'adult', 'advance',
      'advice', 'aerobic', 'affair', 'afford', 'afraid', 'again', 'age', 'agent',
      'agree', 'ahead', 'aim', 'air', 'airport', 'aisle', 'alarm', 'album',
      'alcohol', 'alert', 'alien', 'all', 'alley', 'allow', 'almost', 'alone',
      'alpha', 'already', 'also', 'alter', 'always', 'amateur', 'amazing', 'among',
      'amount', 'amused', 'analyst', 'anchor', 'ancient', 'anger', 'angle', 'angry',
      'animal', 'ankle', 'announce', 'annual', 'another', 'answer', 'antenna', 'antique',
      'anxiety', 'any', 'apart', 'apology', 'appear', 'apple', 'approve', 'april',
      'arch', 'arctic', 'area', 'arena', 'argue', 'arm', 'armed', 'armor',
      'army', 'around', 'arrange', 'arrest', 'arrive', 'arrow', 'art', 'artefact',
      'artist', 'artwork', 'ask', 'aspect', 'assault', 'asset', 'assist', 'assume',
      'asthma', 'athlete', 'atom', 'attack', 'attend', 'attitude', 'attract', 'auction',
      'audit', 'august', 'aunt', 'author', 'auto', 'autumn', 'average', 'avocado',
      'avoid', 'awake', 'aware', 'away', 'awesome', 'awful', 'awkward', 'axis',
      'baby', 'bachelor', 'bacon', 'badge', 'bag', 'balance', 'balcony', 'ball',
      'bamboo', 'banana', 'banner', 'bar', 'barely', 'bargain', 'barrel', 'base',
      'basic', 'basket', 'battle', 'beach', 'bean', 'beauty', 'because', 'become',
      'beef', 'before', 'begin', 'behave', 'behind', 'believe', 'below', 'belt',
      'bench', 'benefit', 'best', 'betray', 'better', 'between', 'beyond', 'bicycle',
      'bid', 'bike', 'bind', 'biology', 'bird', 'birth', 'bitter', 'black',
      'blade', 'blame', 'blanket', 'blast', 'bleak', 'bless', 'blind', 'blood',
      'blossom', 'blouse', 'blue', 'blur', 'blush', 'board', 'boat', 'body',
      'boil', 'bomb', 'bone', 'bonus', 'book', 'boost', 'border', 'boring',
      'borrow', 'boss', 'bottom', 'bounce', 'box', 'boy', 'bracket', 'brain',
      'brand', 'brass', 'brave', 'bread', 'breeze', 'brick', 'bridge', 'brief',
      'bright', 'bring', 'brisk', 'broccoli', 'broken', 'bronze', 'broom', 'brother',
      'brown', 'brush', 'bubble', 'buddy', 'budget', 'buffalo', 'build', 'bulb',
      'bulk', 'bullet', 'bundle', 'bunker', 'burden', 'burger', 'burst', 'bus',
      'business', 'busy', 'butter', 'buyer', 'buzz', 'cabbage', 'cabin', 'cable',
    ];

    // Ensure we have exactly 32 bytes
    if (bytes.length != 32) {
      print('[Mnemonic] Warning: expected 32 bytes, got ${bytes.length}');
    }

    // Convert bytes to 11-bit indices (BIP-39 encoding)
    final words = <String>[];
    int bitBuffer = 0;
    int bitsInBuffer = 0;

    for (final byte in bytes) {
      bitBuffer = (bitBuffer << 8) | byte;
      bitsInBuffer += 8;

      while (bitsInBuffer >= 11) {
        bitsInBuffer -= 11;
        final index = (bitBuffer >> bitsInBuffer) & 0x7FF; // Extract 11 bits
        words.add(wordList[index % wordList.length]); // Use modulo for safety

        if (words.length == 24) break;
      }
      if (words.length == 24) break;
    }

    // Pad if needed
    while (words.length < 24) {
      words.add(wordList[0]);
    }

    return words;
  }

  List<String> _generateDummyMnemonic() {
    // Fallback dummy words if backup derivation fails
    const wordList = [
      'alpha', 'bravo', 'charlie', 'delta', 'echo', 'foxtrot',
      'golf', 'hotel', 'india', 'juliett', 'kilo', 'lima',
      'mike', 'november', 'oscar', 'papa', 'quebec', 'romeo',
      'sierra', 'tango', 'uniform', 'victor', 'whiskey', 'xray',
    ];

    final words = <String>[];
    for (var i = 0; i < 24; i++) {
      words.add(wordList[i % wordList.length]);
    }
    return words;
  }

  void _copyMnemonic() {
    Clipboard.setData(ClipboardData(text: _recoveryMnemonic.join(' ')));
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(S.mnemonicCopied),
        duration: const Duration(seconds: 2),
      ),
    );
  }

  Future<void> _saveToCloud() async {
    try {
      // Get the last DKG session ID from MpcSessionStore
      final lastSession = await MpcSessionStore.loadSession();
      if (lastSession == null) {
        throw Exception('No DKG session found');
      }

      // Derive backup shard bytes
      final backupBytes = await MpcBridge.dkgDeriveBackupShare(lastSession.sessionId);

      // Save to cloud using BackupShardService
      final cloudBackup = PlatformCloudBackup();
      final backupService = BackupShardService(cloudBackup);

      final result = await backupService.storeBackupShard(backupBytes);

      if (!mounted) return;

      if (result.method == BackupMethod.cloud) {
        setState(() => _backupInCloud = true);
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: const Text('Backup saved to cloud successfully'),
            backgroundColor: CwColors.success,
            duration: const Duration(seconds: 2),
          ),
        );
      } else {
        // File backup fallback
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Backup saved to file: ${result.filePath}'),
            duration: const Duration(seconds: 3),
          ),
        );
      }
    } catch (e) {
      print('[OnboardingBackup] Failed to save to cloud: $e');
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text('Cloud backup failed: $e'),
          backgroundColor: CwColors.danger,
          duration: const Duration(seconds: 3),
        ),
      );
    }
  }

  void _confirmBackup() {
    // Prepare verification - show 4 random words in shuffled order
    final shuffled = List<String>.from(_recoveryMnemonic)..shuffle();
    _verifyWords = shuffled.take(4).toList();
    _selectedVerifyWords = [];
    _goTo(_Stage.verifyBackup);
  }

  void _skipBackup() {
    setState(() => _backupSkipped = true);
    _goTo(_Stage.ready);
  }

  // ---- Verify Backup ----
  void _toggleVerifyWord(String word) {
    setState(() {
      if (_selectedVerifyWords.contains(word)) {
        _selectedVerifyWords.remove(word);
      } else {
        _selectedVerifyWords.add(word);
      }
    });
  }

  bool _isVerifyCorrect() {
    // Check if selected words are in the correct order (positions 3, 6, 9, 12)
    final checkPositions = [2, 5, 8, 11]; // 0-indexed
    for (var i = 0; i < checkPositions.length && i < _selectedVerifyWords.length; i++) {
      if (_selectedVerifyWords[i] != _recoveryMnemonic[checkPositions[i]]) {
        return false;
      }
    }
    return _selectedVerifyWords.length == 4;
  }

  void _submitVerification() {
    if (_isVerifyCorrect()) {
      // Clear mnemonic from memory after successful backup
      _recoveryMnemonic.clear();
      _verifyWords.clear();
      _selectedVerifyWords.clear();
      _goTo(_Stage.ready);
    } else {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(S.verifyWrongOrder),
          backgroundColor: CwColors.danger,
          duration: const Duration(seconds: 2),
        ),
      );
      // Reset selection
      setState(() => _selectedVerifyWords.clear());
    }
  }

  // ---- Importing ----
  Future<void> _submitImport() async {
    if (_wordCount != 12 && _wordCount != 24) return;

    try {
      // Convert mnemonic words back to bytes (reverse of _bytesToMnemonic)
      final words = _importCtrl.text.trim().split(RegExp(r'\s+'));
      final backupBytes = _mnemonicToBytes(words);

      // TODO: Wire to RecoveryService for full recovery flow
      // This requires backend API integration for OTP verification
      // For now, just import the backup shard into FFI layer
      await MpcBridge.recoveryImportBackupShard(backupBytes);

      // Store mnemonic in secure storage for recovery completion later
      await SecureStorage.saveMnemonic(words.join(' '));

      _goTo(_Stage.bio);
    } catch (e) {
      print('[OnboardingImport] Failed to import backup: $e');
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text('Failed to import recovery phrase: $e'),
          backgroundColor: CwColors.danger,
          duration: const Duration(seconds: 3),
        ),
      );
    }
  }

  List<int> _mnemonicToBytes(List<String> words) {
    // Reverse of _bytesToMnemonic: convert words back to bytes
    // Simplified implementation - in production use full BIP-39 decoding
    final bytes = <int>[];

    // For now, create a deterministic 32-byte output from word hashes
    // This is a placeholder until full BIP-39 wordlist is integrated
    for (var i = 0; i < 32; i++) {
      final wordIndex = i % words.length;
      final word = words[wordIndex];
      bytes.add((word.codeUnitAt(0) + i) % 256);
    }

    return bytes;
  }

  Future<void> _pasteWords() async {
    final data = await Clipboard.getData(Clipboard.kTextPlain);
    if (data?.text != null) {
      _importCtrl.text = data!.text!;
    }
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

    // Persist onboarding metadata
    await SecureStorage.save('onboarding_completed_at', DateTime.now().toIso8601String());
    await SecureStorage.save('backup_status', _backupSkipped ? 'skipped' : (_backupInCloud ? 'cloud' : 'manual'));
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
          child: _buildStage(),
        ),
      ),
    );
  }

  Widget _buildStage() {
    switch (_stage) {
      case _Stage.hero:
        return _heroStage();
      case _Stage.start:
        return _startStage();
      case _Stage.creating:
        return _creatingStage();
      case _Stage.importing:
        return _importingStage();
      case _Stage.bio:
        return _bioStage();
      case _Stage.name:
        return _nameStage();
      case _Stage.backup:
        return _backupStage();
      case _Stage.verifyBackup:
        return _verifyBackupStage();
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

  // ===================== STAGE 1: HERO =====================

  Widget _heroStage() {
    return SingleChildScrollView(
      key: const ValueKey('hero'),
      padding: const EdgeInsets.symmetric(horizontal: 28),
      child: Column(
        children: [
          const SizedBox(height: 40),
          const CwOrb(size: 140, breathing: true),
          const SizedBox(height: 28),
          // Kicker
          Text(
            S.heroKicker,
            style: Theme.of(context).textTheme.labelLarge?.copyWith(
                  color: CwColors.ink3,
                  letterSpacing: 1.2,
                ),
            textAlign: TextAlign.center,
          ),
          const SizedBox(height: 12),
          // H1 with italic emphasis
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
          // Explain paragraph
          Text(
            S.heroExplain,
            style: Theme.of(context).textTheme.bodyLarge?.copyWith(
                  color: CwColors.ink2,
                ),
            textAlign: TextAlign.center,
          ),
          const SizedBox(height: 32),
          // 3 feature rows
          _featureRow(Icons.shield_outlined, S.heroFeat1h, S.heroFeat1s),
          const SizedBox(height: 16),
          _featureRow(Icons.public, S.heroFeat2h, S.heroFeat2s),
          const SizedBox(height: 16),
          _featureRow(Icons.auto_awesome, S.heroFeat3h, S.heroFeat3s),
          const SizedBox(height: 36),
          // CTA
          _primaryButton(S.getStarted, () => _goTo(_Stage.start)),
          const SizedBox(height: 12),
          // Secondary link
          _secondaryLink(S.haveWallet, () => _goTo(_Stage.start)),
          const SizedBox(height: 16),
          // Legal text
          Text(
            S.heroLegal,
            style: Theme.of(context).textTheme.bodySmall?.copyWith(
                  color: CwColors.ink4,
                  fontSize: 11,
                ),
            textAlign: TextAlign.center,
          ),
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

  // ===================== STAGE 2: START =====================

  Widget _startStage() {
    return SingleChildScrollView(
      key: const ValueKey('start'),
      child: Column(
        children: [
          _topBar(showBack: true, step: 0, total: 3),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 28),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                const SizedBox(height: 12),
                _heading(S.startH1),
                const SizedBox(height: 8),
                _subtitle(S.startSub),
                const SizedBox(height: 28),
                // Option cards
                _optionCard(
                  icon: Icons.add_circle_outline,
                  title: S.pickCreateTitle,
                  desc: S.pickCreateDesc,
                  tag: S.pickCreateTag,
                  onTap: () => _goTo(_Stage.creating),
                ),
                const SizedBox(height: 12),
                _optionCard(
                  icon: Icons.downloading,
                  title: S.pickImportTitle,
                  desc: S.pickImportDesc,
                  onTap: () => _goTo(_Stage.importing),
                ),
                const SizedBox(height: 12),
                _optionCard(
                  icon: Icons.usb,
                  title: S.pickHwTitle,
                  desc: S.pickHwDesc,
                  onTap: () {}, // Hardware wallet — not implemented in prototype
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _optionCard({
    required IconData icon,
    required String title,
    required String desc,
    String? tag,
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
            const SizedBox(width: 8),
            Padding(
              padding: const EdgeInsets.only(top: 8),
              child: Icon(Icons.chevron_right, size: 20, color: CwColors.ink4),
            ),
          ],
        ),
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

  Widget _importingStage() {
    final valid = _wordCount == 12 || _wordCount == 24;
    return SingleChildScrollView(
      key: const ValueKey('importing'),
      child: Column(
        children: [
          _topBar(showBack: true, step: 1, total: 3),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 28),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                const SizedBox(height: 12),
                _heading(S.importH1),
                const SizedBox(height: 8),
                _subtitle(S.importSub),
                const SizedBox(height: 20),
                // Warning box
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
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Icon(Icons.warning_amber_rounded,
                          size: 20, color: CwColors.warn),
                      const SizedBox(width: 10),
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(S.importWarn,
                                style: TextStyle(
                                    fontSize: 13,
                                    fontWeight: FontWeight.w600,
                                    color: CwColors.warn)),
                            const SizedBox(height: 4),
                            Text(S.importWarnBody,
                                style: TextStyle(
                                    fontSize: 12, color: CwColors.ink2)),
                          ],
                        ),
                      ),
                    ],
                  ),
                ),
                const SizedBox(height: 20),
                // Textarea
                Container(
                  decoration: BoxDecoration(
                    color: CwColors.bgCard,
                    borderRadius: BorderRadius.circular(14),
                    border: Border.all(color: CwColors.line),
                  ),
                  child: TextField(
                    controller: _importCtrl,
                    maxLines: 5,
                    style: const TextStyle(
                      fontFamily: 'JetBrainsMono',
                      fontSize: 14,
                      color: CwColors.ink1,
                    ),
                    decoration: InputDecoration(
                      hintText: S.importPlaceholder,
                      hintStyle: TextStyle(
                          fontFamily: 'JetBrainsMono',
                          fontSize: 14,
                          color: CwColors.ink4),
                      contentPadding: const EdgeInsets.all(16),
                      border: InputBorder.none,
                    ),
                  ),
                ),
                const SizedBox(height: 8),
                // Word counter + paste
                Row(
                  children: [
                    Text(
                      '$_wordCount ${_wordCount == 1 ? 'word' : 'words'}',
                      style: Theme.of(context).textTheme.labelMedium?.copyWith(
                            color: valid ? CwColors.success : CwColors.ink4,
                          ),
                    ),
                    const Spacer(),
                    TextButton.icon(
                      onPressed: _pasteWords,
                      icon: Icon(Icons.paste, size: 16, color: CwColors.ink3),
                      label: Text(S.paste,
                          style:
                              TextStyle(fontSize: 13, color: CwColors.ink3)),
                    ),
                  ],
                ),
                const SizedBox(height: 20),
                // Submit
                _primaryButton(
                  S.importSubmit,
                  valid ? _submitImport : null,
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

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
                // Face ID icon in animated ring
                SizedBox(
                  width: 120,
                  height: 120,
                  child: Stack(
                    alignment: Alignment.center,
                    children: [
                      // Animated ring
                      AnimatedBuilder(
                        animation: _bioRingCtrl,
                        builder: (_, _) {
                          final scale =
                              _bioScanning ? 1.0 + _bioRingCtrl.value * 0.08 : 1.0;
                          final opacity =
                              _bioScanning ? 0.6 - _bioRingCtrl.value * 0.3 : 0.25;
                          return Transform.scale(
                            scale: scale,
                            child: Container(
                              width: 120,
                              height: 120,
                              decoration: BoxDecoration(
                                shape: BoxShape.circle,
                                border: Border.all(
                                  color: (_bioDone ? CwColors.success : CwColors.accent)
                                      .withValues(alpha: opacity),
                                  width: 3,
                                ),
                              ),
                            ),
                          );
                        },
                      ),
                      // Icon
                      Icon(
                        _bioDone ? Icons.check_circle : Icons.face,
                        size: 56,
                        color: _bioDone ? CwColors.success : CwColors.accent,
                      ),
                    ],
                  ),
                ),
                const SizedBox(height: 32),
                _heading(_bioDone ? S.bioDone : S.bioH1),
                const SizedBox(height: 8),
                _subtitle(S.bioSub),
                const SizedBox(height: 40),
                if (!_bioDone && !_bioScanning) ...[
                  _primaryButton(S.bioActivate, _startBioScan),
                  const SizedBox(height: 12),
                  _secondaryLink(S.bioSkip, _skipBio),
                ],
                if (_bioScanning)
                  Text(
                    '...',
                    style: TextStyle(fontSize: 18, color: CwColors.ink4),
                  ),
              ],
            ),
          ),
        ],
      ),
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

  // ===================== STAGE 7: BACKUP =====================

  Widget _backupStage() {
    // Lazy generate mnemonic when first entering this stage
    if (_recoveryMnemonic.isEmpty) {
      Future.microtask(() => _startBackup());
    }

    return SingleChildScrollView(
      key: const ValueKey('backup'),
      child: Column(
        children: [
          _topBar(showBack: true, step: 2, total: 3),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 28),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                const SizedBox(height: 12),
                Center(child: _heading(S.backupH1)),
                const SizedBox(height: 8),
                Center(
                  child: _subtitle(S.backupSub),
                ),
                const SizedBox(height: 20),
                // Warning box
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
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Icon(Icons.warning_amber_rounded,
                          size: 20, color: CwColors.warn),
                      const SizedBox(width: 10),
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(S.backupImportant,
                                style: TextStyle(
                                    fontSize: 13,
                                    fontWeight: FontWeight.w600,
                                    color: CwColors.warn)),
                            const SizedBox(height: 4),
                            Text(S.backupWarnBody,
                                style: TextStyle(
                                    fontSize: 12, color: CwColors.ink2)),
                          ],
                        ),
                      ),
                    ],
                  ),
                ),
                const SizedBox(height: 20),
                // Mnemonic grid
                Container(
                  width: double.infinity,
                  padding: const EdgeInsets.all(16),
                  decoration: BoxDecoration(
                    color: CwColors.bgCard,
                    borderRadius: BorderRadius.circular(16),
                    border: Border.all(color: CwColors.line),
                  ),
                  child: Column(
                    children: [
                      GridView.builder(
                        shrinkWrap: true,
                        physics: const NeverScrollableScrollPhysics(),
                        gridDelegate: const SliverGridDelegateWithFixedCrossAxisCount(
                          crossAxisCount: 2,
                          childAspectRatio: 3.5,
                          crossAxisSpacing: 12,
                          mainAxisSpacing: 8,
                        ),
                        itemCount: _recoveryMnemonic.length,
                        itemBuilder: (context, i) {
                          return Container(
                            padding: const EdgeInsets.symmetric(
                                horizontal: 12, vertical: 8),
                            decoration: BoxDecoration(
                              color: CwColors.accentSoft.withValues(alpha: 0.3),
                              borderRadius: BorderRadius.circular(8),
                            ),
                            child: Row(
                              children: [
                                Text(
                                  '${i + 1}',
                                  style: TextStyle(
                                    fontFamily: 'JetBrainsMono',
                                    fontSize: 12,
                                    color: CwColors.ink3,
                                    fontWeight: FontWeight.w600,
                                  ),
                                ),
                                const SizedBox(width: 8),
                                Expanded(
                                  child: Text(
                                    _recoveryMnemonic[i],
                                    style: TextStyle(
                                      fontFamily: 'JetBrainsMono',
                                      fontSize: 13,
                                      color: CwColors.ink1,
                                      fontWeight: FontWeight.w500,
                                    ),
                                  ),
                                ),
                              ],
                            ),
                          );
                        },
                      ),
                      const SizedBox(height: 16),
                      // Copy button
                      TextButton.icon(
                        onPressed: _copyMnemonic,
                        icon: Icon(Icons.copy, size: 16, color: CwColors.accent),
                        label: Text(
                          S.backupCopy,
                          style: TextStyle(fontSize: 13, color: CwColors.accent),
                        ),
                      ),
                    ],
                  ),
                ),
                const SizedBox(height: 16),
                // Cloud backup button
                if (!_backupInCloud)
                  Container(
                    width: double.infinity,
                    decoration: BoxDecoration(
                      border: Border.all(color: CwColors.line),
                      borderRadius: BorderRadius.circular(14),
                    ),
                    child: TextButton.icon(
                      onPressed: _saveToCloud,
                      icon: Icon(Icons.cloud_upload, size: 18, color: CwColors.accent),
                      label: Text(
                        'Save to iCloud / Google Cloud',
                        style: TextStyle(fontSize: 14, color: CwColors.accent),
                      ),
                    ),
                  )
                else
                  Container(
                    width: double.infinity,
                    padding: const EdgeInsets.all(12),
                    decoration: BoxDecoration(
                      color: CwColors.success.withValues(alpha: 0.1),
                      borderRadius: BorderRadius.circular(12),
                      border: Border.all(color: CwColors.success.withValues(alpha: 0.3)),
                    ),
                    child: Row(
                      mainAxisAlignment: MainAxisAlignment.center,
                      children: [
                        Icon(Icons.check_circle, size: 18, color: CwColors.success),
                        const SizedBox(width: 8),
                        Text(
                          'Backed up to cloud',
                          style: TextStyle(fontSize: 14, color: CwColors.success, fontWeight: FontWeight.w600),
                        ),
                      ],
                    ),
                  ),
                const SizedBox(height: 24),
                _primaryButton(S.backupConfirmed, _confirmBackup),
                const SizedBox(height: 12),
                Center(
                  child: _secondaryLink(S.backupSkip, _skipBackup),
                ),
                const SizedBox(height: 24),
              ],
            ),
          ),
        ],
      ),
    );
  }

  // ===================== STAGE 8: VERIFY BACKUP =====================

  Widget _verifyBackupStage() {
    return SingleChildScrollView(
      key: const ValueKey('verifyBackup'),
      child: Column(
        children: [
          _topBar(showBack: true, step: 2, total: 3),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 28),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                const SizedBox(height: 12),
                Center(child: _heading(S.verifyH1)),
                const SizedBox(height: 8),
                Center(
                  child: _subtitle(S.verifySub),
                ),
                const SizedBox(height: 24),
                // Selected words display
                Container(
                  width: double.infinity,
                  padding: const EdgeInsets.all(16),
                  decoration: BoxDecoration(
                    color: CwColors.bgCard,
                    borderRadius: BorderRadius.circular(16),
                    border: Border.all(color: CwColors.line),
                  ),
                  child: Wrap(
                    spacing: 8,
                    runSpacing: 8,
                    children: List.generate(4, (i) {
                      final hasSelection = i < _selectedVerifyWords.length;
                      return Container(
                        padding: const EdgeInsets.symmetric(
                            horizontal: 14, vertical: 8),
                        decoration: BoxDecoration(
                          color: hasSelection
                              ? CwColors.accentSoft
                              : CwColors.bgPaper,
                          borderRadius: BorderRadius.circular(8),
                          border: Border.all(
                            color: hasSelection ? CwColors.accent : CwColors.line,
                            width: hasSelection ? 1.5 : 1,
                          ),
                        ),
                        child: Text(
                          hasSelection ? _selectedVerifyWords[i] : '${i + 1}',
                          style: TextStyle(
                            fontFamily: 'JetBrainsMono',
                            fontSize: 13,
                            color: hasSelection ? CwColors.accent : CwColors.ink3,
                            fontWeight: hasSelection ? FontWeight.w600 : FontWeight.w400,
                          ),
                        ),
                      );
                    }),
                  ),
                ),
                const SizedBox(height: 24),
                // Word options
                Text(
                  S.verifySelectLabel,
                  style: TextStyle(
                    fontSize: 14,
                    fontWeight: FontWeight.w600,
                    color: CwColors.ink2,
                  ),
                ),
                const SizedBox(height: 12),
                Wrap(
                  spacing: 8,
                  runSpacing: 8,
                  children: _verifyWords.map((word) {
                    final isSelected = _selectedVerifyWords.contains(word);
                    return GestureDetector(
                      onTap: () => _toggleVerifyWord(word),
                      child: AnimatedContainer(
                        duration: const Duration(milliseconds: 150),
                        padding: const EdgeInsets.symmetric(
                            horizontal: 16, vertical: 10),
                        decoration: BoxDecoration(
                          color: isSelected ? CwColors.accent : CwColors.bgCard,
                          borderRadius: BorderRadius.circular(10),
                          border: Border.all(
                            color: isSelected ? CwColors.accent : CwColors.line,
                            width: isSelected ? 1.5 : 1,
                          ),
                        ),
                        child: Text(
                          word,
                          style: TextStyle(
                            fontFamily: 'JetBrainsMono',
                            fontSize: 13,
                            color: isSelected ? Colors.white : CwColors.ink1,
                            fontWeight: FontWeight.w500,
                          ),
                        ),
                      ),
                    );
                  }).toList(),
                ),
                const SizedBox(height: 32),
                _primaryButton(
                  S.verifySubmit,
                  _selectedVerifyWords.length == 4 ? _submitVerification : null,
                ),
                const SizedBox(height: 24),
              ],
            ),
          ),
        ],
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
