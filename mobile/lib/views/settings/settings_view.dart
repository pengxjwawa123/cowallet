import 'package:flutter/material.dart';
import '../../theme/colors.dart';
import '../../l10n/strings.dart';
import '../../widgets/cw_chip.dart';
import '../../widgets/section_label.dart';
import '../../main.dart';
import '../../services/locator.dart';

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

  @override
  void initState() {
    super.initState();
    _loadBiometricStatus();
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

    if (value) {
      // Enable: first authenticate to confirm
      final authenticated = await Services.biometrics.authenticate(
        reason: S.biometricAuthReason,
      );
      if (!authenticated) return;
    }

    await Services.biometrics.setEnabled(value);
    if (mounted) {
      setState(() => _biometricEnabled = value);
    }
  }

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      child: ListView(
        padding: const EdgeInsets.fromLTRB(20, 8, 20, 40),
        children: [
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
            S.onEnter,
            style: const TextStyle(fontSize: 11, color: CwColors.ink3),
          ),
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
            label: S.on,
            variant: ChipVariant.green,
          ),
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
        ),
        const Divider(indent: 52, height: 1),
        _settingRow(
          context,
          icon: Icons.bar_chart_rounded,
          iconColor: CwColors.ink3,
          iconBg: CwColors.bgSubtle,
          title: S.weeklyReport,
          subtitle: S.weeklyReportSub,
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
