import 'package:flutter/material.dart';
import '../../theme/colors.dart';
import '../../l10n/strings.dart';
import '../../widgets/cw_chip.dart';
import '../../widgets/section_label.dart';
import '../../widgets/top_toast.dart';
import '../../main.dart';
import '../../services/locator.dart';
import '../../services/settings_service.dart';
import '../../utils/secure_storage.dart';

class SettingsView extends StatefulWidget {
  const SettingsView({super.key});

  @override
  State<SettingsView> createState() => _SettingsViewState();
}

class _SettingsViewState extends State<SettingsView> {
  bool _biometricEnabled = false;
  bool _biometricAvailable = false;
  bool _hasEnrolledBiometrics = false;
  String _biometricType = 'Biometric';
  bool _autoRotateEnabled = false;
  String? _lastRotationDate;
  bool _isRotating = false;

  SettingsService get _settings => Services.settings;

  @override
  void initState() {
    super.initState();
    _loadBiometricStatus();
    _loadKeySecuritySettings();
    _settings.addListener(_onSettingsChanged);
  }

  @override
  void dispose() {
    _settings.removeListener(_onSettingsChanged);
    super.dispose();
  }

  void _onSettingsChanged() {
    if (mounted) setState(() {});
  }

  Future<void> _loadBiometricStatus() async {
    final available = await Services.biometrics.isAvailable();
    final enabled = await Services.biometrics.isEnabled();
    final hasEnrolled = await Services.biometrics.hasEnrolledBiometrics();
    final bioType = await Services.biometrics.getPrimaryBiometricType();

    if (mounted) {
      setState(() {
        _biometricAvailable = available;
        _biometricEnabled = enabled;
        _hasEnrolledBiometrics = hasEnrolled;
        _biometricType = bioType;
      });
    }
  }

  String _getBiometricSubtitle() {
    if (!_biometricAvailable) {
      return S.biometricNotAvailable;
    }
    if (!_hasEnrolledBiometrics) {
      return 'Please set up $_biometricType in your device settings first';
    }
    return 'Use $_biometricType to verify sensitive operations';
  }

  Future<void> _toggleBiometric(bool value) async {
    if (!_biometricAvailable || !_hasEnrolledBiometrics) return;

    final authenticated = await Services.biometrics.authenticate(
      reason: S.biometricAuthReason,
    );
    if (!authenticated) return;

    await Services.biometrics.setEnabled(value);
    if (mounted) {
      setState(() => _biometricEnabled = value);
    }
  }

  Future<void> _toggleEmergencyFreeze() async {
    if (_settings.emergencyFreezeActive) {
      // Deactivating — no confirmation needed
      await _settings.setEmergencyFreezeActive(false);
      if (mounted) {
        showTopToast(context, S.emergencyFreezeDeactivated, backgroundColor: CwColors.success);
      }
    } else {
      // Activating — show confirmation dialog
      final confirmed = await showDialog<bool>(
        context: context,
        builder: (ctx) => AlertDialog(
          title: Text(S.emergencyFreezeConfirmTitle),
          content: Text(S.emergencyFreezeConfirmBody),
          actions: [
            TextButton(
              onPressed: () => Navigator.pop(ctx, false),
              child: Text(S.cancel),
            ),
            TextButton(
              onPressed: () => Navigator.pop(ctx, true),
              style: TextButton.styleFrom(foregroundColor: CwColors.danger),
              child: Text(S.confirm),
            ),
          ],
        ),
      );
      if (confirmed == true) {
        await _settings.setEmergencyFreezeActive(true);
        if (mounted) {
          showTopToast(context, S.emergencyFreezeActivated, backgroundColor: CwColors.danger);
        }
      }
    }
  }

  void _toggleLanguage() {
    final newLang = S.lang == Lang.zh ? Lang.en : Lang.zh;
    _settings.setLanguage(newLang == Lang.zh ? 'zh' : 'en');
    CowalletApp.of(context).setLang(newLang);
  }

  void _toggleIntentMode() {
    final newMode = _settings.intentMode == IntentMode.onEnter
        ? IntentMode.whileTyping
        : IntentMode.onEnter;
    _settings.setIntentMode(newMode);
  }

  void _toggleVoiceInput() {
    _settings.setVoiceInputEnabled(!_settings.voiceInputEnabled);
  }

  void _toggleWeeklyReport() {
    _settings.setWeeklyReportEnabled(!_settings.weeklyReportEnabled);
  }

  Future<void> _loadKeySecuritySettings() async {
    final autoRotate = await SecureStorage.get('auto_rotate_keys');
    final lastRotation = await SecureStorage.get('last_key_rotation');

    if (mounted) {
      setState(() {
        _autoRotateEnabled = autoRotate == 'true';
        _lastRotationDate = lastRotation;
      });
    }
  }

  Future<void> _toggleAutoRotate(bool value) async {
    await SecureStorage.save('auto_rotate_keys', value.toString());
    if (mounted) {
      setState(() => _autoRotateEnabled = value);
    }
  }

  Future<void> _performKeyRotation() async {
    if (_isRotating) return;

    setState(() => _isRotating = true);

    try {
      final walletAddress = await Services.mpcWallet.getAddress();
      await Services.mpcWallet.runReshare(walletId: walletAddress);

      final now = DateTime.now().toIso8601String();
      await SecureStorage.save('last_key_rotation', now);

      if (mounted) {
        setState(() {
          _lastRotationDate = now;
          _isRotating = false;
        });

        showTopToast(context, S.rotationSuccess, backgroundColor: CwColors.success);
      }
    } catch (e) {
      if (mounted) {
        setState(() => _isRotating = false);

        showTopToast(context, '${S.rotationFailed}: $e', backgroundColor: CwColors.danger);
      }
    }
  }

  String _formatLastRotation() {
    if (_lastRotationDate == null) return S.never;

    try {
      final date = DateTime.parse(_lastRotationDate!);
      final now = DateTime.now();
      final diff = now.difference(date);

      if (diff.inDays == 0) {
        return S.today;
      } else if (diff.inDays == 1) {
        return S.lang == Lang.zh ? '昨天' : 'Yesterday';
      } else if (diff.inDays < 30) {
        return S.lang == Lang.zh ? '${diff.inDays} 天前' : '${diff.inDays} days ago';
      } else {
        final months = (diff.inDays / 30).floor();
        return S.lang == Lang.zh ? '$months 个月前' : '$months months ago';
      }
    } catch (e) {
      return S.never;
    }
  }

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      child: ListView(
        padding: const EdgeInsets.fromLTRB(20, 8, 20, 40),
        children: [
          // Emergency freeze banner
          if (_settings.emergencyFreezeActive)
            Container(
              margin: const EdgeInsets.only(bottom: 10),
              padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
              decoration: BoxDecoration(
                color: CwColors.danger.withValues(alpha: 0.12),
                borderRadius: BorderRadius.circular(12),
                border: Border.all(color: CwColors.danger.withValues(alpha: 0.4)),
              ),
              child: Row(
                children: [
                  const Icon(Icons.ac_unit, size: 18, color: CwColors.danger),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      S.frozenBanner,
                      style: const TextStyle(
                        fontSize: 12,
                        fontWeight: FontWeight.w600,
                        color: CwColors.danger,
                      ),
                    ),
                  ),
                ],
              ),
            ),
          // Header
          Padding(
            padding: const EdgeInsets.only(top: 8, bottom: 4),
            child: Text(S.settings,
                style: Theme.of(context).textTheme.titleLarge),
          ),

          // ── Section: 安全 ──
          SectionLabel(title: S.security),
          _keysCard(context),
          const SizedBox(height: 10),
          _securityList(context),

          // ── Section: 密钥安全 ──
          SectionLabel(title: S.keySecurity),
          _keySecurityList(context),

          // ── Section: 对话 ──
          SectionLabel(title: S.conversation),
          _conversationList(context),

          // ── Section: 一般 ──
          SectionLabel(title: S.general),
          _generalList(context),

          // ── Signoff ──
          const SizedBox(height: 28),
          Center(
            child: Text(
              S.signoff1,
              style: const TextStyle(
                fontFamily: 'JetBrainsMono',
                fontSize: 10,
                color: CwColors.ink4,
              ),
            ),
          ),
          const SizedBox(height: 2),
          Center(
            child: Text(
              S.signoff2,
              style: const TextStyle(
                fontFamily: 'JetBrainsMono',
                fontSize: 10,
                color: CwColors.ink4,
              ),
            ),
          ),
        ],
      ),
    );
  }

  // ── Keys health card ──
  Widget _keysCard(BuildContext context) {
    return GestureDetector(
      onTap: () => Navigator.pushNamed(context, '/keys'),
      child: Container(
        padding: const EdgeInsets.all(14),
        decoration: BoxDecoration(
          color: CwColors.bgCard,
          borderRadius: BorderRadius.circular(18),
          border: Border.all(color: CwColors.line),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            // Top row: title + chip
            Row(
              children: [
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        S.keysCheckup,
                        style: const TextStyle(
                          fontFamily: 'NotoSerifSC',
                          fontSize: 13.5,
                          fontWeight: FontWeight.w600,
                          color: CwColors.ink1,
                        ),
                      ),
                      const SizedBox(height: 2),
                      Text(
                        S.keysCheckupSub,
                        style: const TextStyle(
                          fontSize: 11,
                          color: CwColors.ink3,
                        ),
                      ),
                    ],
                  ),
                ),
                CwChip(
                  label: S.allSafe,
                  variant: ChipVariant.green,
                  showDot: true,
                ),
              ],
            ),
            const SizedBox(height: 14),
            // 3-column grid
            Row(
              children: [
                _keyIndicator(
                  icon: Icons.phone_iphone,
                  label: S.onPhone,
                  color: CwColors.success,
                  bgColor: CwColors.successSoft,
                ),
                const SizedBox(width: 10),
                _keyIndicator(
                  icon: Icons.cloud_outlined,
                  label: S.inCloud,
                  color: CwColors.success,
                  bgColor: CwColors.successSoft,
                ),
                const SizedBox(width: 10),
                _keyIndicator(
                  icon: Icons.lock_outline,
                  label: S.recovery,
                  color: CwColors.warn,
                  bgColor: CwColors.warnSoft,
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  Widget _keyIndicator({
    required IconData icon,
    required String label,
    required Color color,
    required Color bgColor,
  }) {
    return Expanded(
      child: Column(
        children: [
          Container(
            width: 30,
            height: 30,
            decoration: BoxDecoration(
              color: bgColor,
              borderRadius: BorderRadius.circular(8),
            ),
            child: Icon(icon, size: 16, color: color),
          ),
          const SizedBox(height: 4),
          Text(
            label,
            style: const TextStyle(fontSize: 11, color: CwColors.ink3),
            textAlign: TextAlign.center,
          ),
        ],
      ),
    );
  }

  // ── Security settings list ──
  Widget _securityList(BuildContext context) {
    return _settingsContainer(
      children: [
        _settingRow(
          context,
          icon: Icons.fingerprint,
          iconColor: CwColors.accent,
          iconBg: CwColors.accentSoft,
          title: S.biometricAuth,
          subtitle: _getBiometricSubtitle(),
          trailing: Switch(
            value: _biometricEnabled && _biometricAvailable && _hasEnrolledBiometrics,
            onChanged: _biometricAvailable && _hasEnrolledBiometrics ? _toggleBiometric : null,
            activeThumbColor: CwColors.accent,
          ),
        ),
        const Divider(indent: 52, height: 1),
        _settingRow(
          context,
          icon: Icons.error_outline,
          iconColor: CwColors.danger,
          iconBg: CwColors.dangerSoft,
          title: S.emergencyFreeze,
          subtitle: S.emergencyFreezeSub,
          trailing: Switch(
            value: _settings.emergencyFreezeActive,
            onChanged: (_) => _toggleEmergencyFreeze(),
            activeTrackColor: CwColors.danger.withValues(alpha: 0.5),
            activeThumbColor: CwColors.danger,
          ),
          onTap: _toggleEmergencyFreeze,
        ),
        const Divider(indent: 52, height: 1),
        _settingRow(
          context,
          icon: Icons.people_outline,
          iconColor: CwColors.warn,
          iconBg: CwColors.warnSoft,
          title: S.emergencyContact,
          subtitle: S.emergencyContactSub,
        ),
        const Divider(indent: 52, height: 1),
        _settingRow(
          context,
          icon: Icons.shield_outlined,
          iconColor: CwColors.info,
          iconBg: CwColors.infoSoft,
          title: S.riskGuard,
          subtitle: S.riskGuardSub,
        ),
      ],
    );
  }

  // ── Conversation settings list ──
  Widget _conversationList(BuildContext context) {
    final intentLabel = _settings.intentMode == IntentMode.onEnter
        ? S.onEnter
        : S.whileTyping;
    return _settingsContainer(
      children: [
        _settingRow(
          context,
          icon: Icons.chat_bubble_outline,
          iconColor: CwColors.ink3,
          iconBg: CwColors.bgSubtle,
          title: S.intentMode,
          subtitle: S.intentModeSub,
          trailing: Text(
            intentLabel,
            style: const TextStyle(fontSize: 11, color: CwColors.ink3),
          ),
          onTap: _toggleIntentMode,
        ),
        const Divider(indent: 52, height: 1),
        _settingRow(
          context,
          icon: Icons.mic_none,
          iconColor: CwColors.ink3,
          iconBg: CwColors.bgSubtle,
          title: S.voiceInput,
          subtitle: S.voiceInputSub,
          trailing: CwChip(
            label: _settings.voiceInputEnabled ? S.on : S.off,
            variant: _settings.voiceInputEnabled
                ? ChipVariant.green
                : ChipVariant.neutral,
          ),
          onTap: _toggleVoiceInput,
        ),
        const Divider(indent: 52, height: 1),
        _settingRow(
          context,
          icon: Icons.auto_awesome_outlined,
          iconColor: CwColors.ink3,
          iconBg: CwColors.bgSubtle,
          title: S.aiModel,
          subtitle: S.aiModelSub,
          trailing: const Icon(Icons.chevron_right,
              size: 18, color: CwColors.ink4),
        ),
      ],
    );
  }

  // ── General settings list ──
  Widget _generalList(BuildContext context) {
    final langLabel = S.lang == Lang.zh ? '中文' : 'English';
    return _settingsContainer(
      children: [
        _settingRow(
          context,
          icon: Icons.language,
          iconColor: CwColors.ink3,
          iconBg: CwColors.bgSubtle,
          title: S.language,
          trailing: Text(
            langLabel,
            style: const TextStyle(fontSize: 11, color: CwColors.ink3),
          ),
          onTap: _toggleLanguage,
        ),
        const Divider(indent: 52, height: 1),
        _settingRow(
          context,
          icon: Icons.bar_chart_rounded,
          iconColor: CwColors.ink3,
          iconBg: CwColors.bgSubtle,
          title: S.weeklyReport,
          subtitle: S.weeklyReportSub,
          trailing: Switch(
            value: _settings.weeklyReportEnabled,
            onChanged: (_) => _toggleWeeklyReport(),
            activeThumbColor: CwColors.accent,
          ),
          onTap: _toggleWeeklyReport,
        ),
        const Divider(indent: 52, height: 1),
        _settingRow(
          context,
          icon: Icons.restart_alt,
          iconColor: CwColors.ink3,
          iconBg: CwColors.bgSubtle,
          title: S.redoOnboarding,
          subtitle: S.redoOnboardingSub,
          trailing: const Text(
            '↻',
            style: TextStyle(fontSize: 16, color: CwColors.ink3),
          ),
          onTap: () {
            CowalletApp.of(context).resetOnboarding();
            Navigator.pushNamedAndRemoveUntil(
                context, '/onboarding', (_) => false);
          },
        ),
      ],
    );
  }

  // ── Shared container for setting groups ──
  Widget _settingsContainer({required List<Widget> children}) {
    return Container(
      decoration: BoxDecoration(
        color: CwColors.bgCard,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: CwColors.line),
      ),
      child: Column(children: children),
    );
  }

  // ── Key Security settings list ──
  Widget _keySecurityList(BuildContext context) {
    return _settingsContainer(
      children: [
        _settingRow(
          context,
          icon: Icons.autorenew,
          iconColor: CwColors.accent,
          iconBg: CwColors.accentSoft,
          title: S.rotateKeyShares,
          subtitle: '${S.lastRotation}: ${_formatLastRotation()}',
          trailing: _isRotating
              ? const SizedBox(
                  width: 20,
                  height: 20,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : IconButton(
                  icon: const Icon(Icons.refresh, size: 20),
                  color: CwColors.accent,
                  onPressed: _performKeyRotation,
                  padding: EdgeInsets.zero,
                  constraints: const BoxConstraints(),
                ),
          onTap: _isRotating ? null : _performKeyRotation,
        ),
        const Divider(indent: 52, height: 1),
        _settingRow(
          context,
          icon: Icons.schedule,
          iconColor: CwColors.info,
          iconBg: CwColors.infoSoft,
          title: S.autoRotate,
          subtitle: S.autoRotateSub,
          trailing: Switch(
            value: _autoRotateEnabled,
            onChanged: _toggleAutoRotate,
            activeThumbColor: CwColors.accent,
          ),
        ),
      ],
    );
  }

  // ── Setting row ──
  Widget _settingRow(
    BuildContext context, {
    required IconData icon,
    required Color iconColor,
    required Color iconBg,
    required String title,
    String? subtitle,
    Widget? trailing,
    VoidCallback? onTap,
  }) {
    return GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTap: onTap ?? () {},
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 12),
        child: Row(
          children: [
            // Leading icon
            Container(
              width: 32,
              height: 32,
              decoration: BoxDecoration(
                color: iconBg,
                borderRadius: BorderRadius.circular(8),
              ),
              child: Icon(icon, size: 17, color: iconColor),
            ),
            const SizedBox(width: 10),
            // Title + subtitle
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    title,
                    style: const TextStyle(
                      fontFamily: 'NotoSerifSC',
                      fontSize: 13.5,
                      fontWeight: FontWeight.w500,
                      color: CwColors.ink1,
                    ),
                  ),
                  if (subtitle != null) ...[
                    const SizedBox(height: 1),
                    Text(
                      subtitle,
                      style: const TextStyle(
                        fontSize: 11,
                        color: CwColors.ink3,
                      ),
                    ),
                  ],
                ],
              ),
            ),
            // Trailing
            ?trailing,
          ],
        ),
      ),
    );
  }
}
