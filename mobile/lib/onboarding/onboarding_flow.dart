import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import '../theme/colors.dart';
import '../widgets/cw_orb.dart';
import '../l10n/strings.dart';
import '../main.dart';
import '../services/locator.dart';

/// The 8 stages of the cowallet onboarding, matching the H5 prototype.
enum _Stage { hero, start, creating, importing, bio, name, ready, persona }

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

  // --- Bio stage state ---
  bool _bioScanning = false;
  bool _bioDone = false;
  late AnimationController _bioRingCtrl;

  // --- Name stage state ---
  final _nameCtrl = TextEditingController();

  bool _createError = false;

  // --- Importing stage state ---
  final _importCtrl = TextEditingController();
  int _wordCount = 0;

  // --- Persona stage state ---
  String? _selectedPersona;

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

  // ---- Creating: wallet generation + animation in parallel ----
  void _startCreating() {
    _createProgress = 0;
    _createChecksDone = 0;
    _createError = false;

    bool walletDone = false;
    bool animDone = false;
    String? generatedAddress;

    void maybeAdvance() {
      if (!walletDone || !animDone || !mounted) return;
      if (generatedAddress != null) {
        CowalletApp.of(context).setWalletAddress(generatedAddress!);
        Future.delayed(const Duration(milliseconds: 400), () {
          if (mounted) _goTo(_Stage.bio);
        });
      }
    }

    // Wallet generation runs concurrently
    Services.wallet.generateWallet().then((keys) {
      generatedAddress = keys.address;
      walletDone = true;
      maybeAdvance();
    }).catchError((Object e) {
      if (!mounted) return;
      _createTimer?.cancel();
      setState(() => _createError = true);
    });

    // Minimum 2-second animation floor
    const tick = Duration(milliseconds: 50);
    _createTimer?.cancel();
    _createTimer = Timer.periodic(tick, (t) {
      setState(() {
        _createProgress += 0.025; // ~2 seconds to reach 1.0
        if (_createProgress >= 0.33 && _createChecksDone < 1) {
          _createChecksDone = 1;
        }
        if (_createProgress >= 0.66 && _createChecksDone < 2) {
          _createChecksDone = 2;
        }
        if (_createProgress >= 1.0) {
          _createProgress = 1.0;
          _createChecksDone = 3;
          t.cancel();
          animDone = true;
          maybeAdvance();
        }
      });
    });
  }

  // ---- Bio animation ----
  void _startBioScan() {
    setState(() => _bioScanning = true);
    _bioRingCtrl.repeat();
    Future.delayed(const Duration(milliseconds: 2600), () {
      if (!mounted) return;
      _bioRingCtrl.stop();
      setState(() {
        _bioScanning = false;
        _bioDone = true;
      });
      Future.delayed(const Duration(milliseconds: 600), () {
        if (mounted) _goTo(_Stage.name);
      });
    });
  }

  void _skipBio() => _goTo(_Stage.name);

  // ---- Name ----
  void _submitName() {
    final name = _nameCtrl.text.trim();
    if (name.isNotEmpty) {
      CowalletApp.of(context).setUserName(name);
    }
    _goTo(_Stage.ready);
  }

  // ---- Importing ----
  void _submitImport() {
    if (_wordCount == 12 || _wordCount == 24) {
      _goTo(_Stage.bio);
    }
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
  void _finish() {
    final appState = CowalletApp.of(context);
    appState.completeOnboarding();
    final addr = appState.walletAddress;
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
                _heading(S.creatingH1),
                const SizedBox(height: 8),
                _subtitle(S.creatingSub),
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

  // ===================== STAGE 7: READY =====================

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
