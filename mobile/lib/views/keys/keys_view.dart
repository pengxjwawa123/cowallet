import 'package:flutter/material.dart';
import '../../theme/colors.dart';
import '../../l10n/strings.dart';
import '../../widgets/cw_chip.dart';
import '../../services/locator.dart';

class KeysView extends StatefulWidget {
  const KeysView({super.key});

  @override
  State<KeysView> createState() => _KeysViewState();
}

class _KeysViewState extends State<KeysView> {
  bool _isAuthenticated = false;

  @override
  void initState() {
    super.initState();
    _checkBiometricStatus();
  }

  Future<void> _checkBiometricStatus() async {
    final available = await Services.biometrics.isAvailable();
    final enabled = await Services.biometrics.isEnabled();
    final hasEnrolled = await Services.biometrics.hasEnrolledBiometrics();
    if (mounted) {
      // If biometric not available, not enabled, or no biometric enrolled,
      // auto-show keys without requiring authentication
      if (!available || !enabled || !hasEnrolled) {
        setState(() => _isAuthenticated = true);
      }
    }
  }

  Future<bool> _authenticate() async {
    if (_isAuthenticated) return true;

    final authenticated = await Services.biometrics.authenticate(
      reason: S.biometricAuthReason,
    );

    if (authenticated && mounted) {
      setState(() => _isAuthenticated = true);
    }
    return authenticated;
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        leading: const BackButton(),
      ),
      body: ListView(
        padding: const EdgeInsets.fromLTRB(20, 0, 20, 40),
        children: [
          // ── Hero ──
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
                  text: '${S.keysH1b}\n',
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

          // ── Subtitle ──
          Text(
            S.keysSub,
            style: const TextStyle(
              fontSize: 13,
              color: CwColors.ink3,
              height: 1.5,
            ),
          ),
          const SizedBox(height: 20),

          // ── 3 Key cards ──
          _keyCard(
            context,
            icon: Icons.phone_iphone,
            iconColor: CwColors.success,
            iconBg: CwColors.successSoft,
            bgColor: CwColors.successSoft,
            borderColor: const Color(0xFFC9D7BC),
            title: S.keyPhone,
            where: S.keyPhoneWhere,
            chipLabel: 'OK',
            chipVariant: ChipVariant.green,
            meta: _isAuthenticated ? S.keyPhoneMeta : '••••••••',
            requireAuth: true,
          ),
          const SizedBox(height: 10),
          _keyCard(
            context,
            icon: Icons.dns_outlined,
            iconColor: CwColors.success,
            iconBg: CwColors.successSoft,
            bgColor: CwColors.successSoft,
            borderColor: const Color(0xFFC9D7BC),
            title: S.keyCloud,
            where: S.keyCloudWhere,
            chipLabel: 'OK',
            chipVariant: ChipVariant.green,
            meta: _isAuthenticated ? S.keyCloudMeta : '••••••••',
            requireAuth: true,
          ),
          const SizedBox(height: 10),
          _keyCard(
            context,
            icon: Icons.lock_outline,
            iconColor: CwColors.warn,
            iconBg: CwColors.warnSoft,
            bgColor: CwColors.warnSoft,
            borderColor: const Color(0xFFE4D2A8),
            title: S.keyRecovery,
            where: S.keyRecoveryWhere,
            chipLabel: S.keyRecoveryTag,
            chipVariant: ChipVariant.amber,
            meta: _isAuthenticated ? S.keyRecoveryMeta : '••••••••',
            actionLabel: S.keyRecoveryAction,
            requireAuth: true,
          ),
          const SizedBox(height: 24),

          // ── Explainer card ──
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
                  style: const TextStyle(
                    fontSize: 12,
                    color: CwColors.ink3,
                    height: 1.6,
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(height: 12),

          // ── Tech detail card ──
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
                  style: const TextStyle(
                    fontSize: 12,
                    color: CwColors.ink3,
                    height: 1.6,
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  // ── Key card ──
  Widget _keyCard(
    BuildContext context, {
    required IconData icon,
    required Color iconColor,
    required Color iconBg,
    required Color bgColor,
    required Color borderColor,
    required String title,
    required String where,
    required String chipLabel,
    required ChipVariant chipVariant,
    required String meta,
    String? actionLabel,
    bool requireAuth = false,
  }) {
    return GestureDetector(
      onTap: requireAuth && !_isAuthenticated
          ? () => _authenticate()
          : null,
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
                // Icon
                Container(
                  width: 40,
                  height: 40,
                  decoration: BoxDecoration(
                    color: iconBg,
                    borderRadius: BorderRadius.circular(10),
                  ),
                  child: Icon(icon, size: 20, color: iconColor),
                ),
                const SizedBox(width: 12),
                // Title + where
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
                      Text(
                        where,
                        style: const TextStyle(
                          fontSize: 11,
                          color: CwColors.ink3,
                        ),
                      ),
                    ],
                  ),
                ),
                // Chip
                CwChip(
                  label: chipLabel,
                  variant: chipVariant,
                  showDot: true,
                ),
              ],
            ),
            const SizedBox(height: 8),
            // Meta
            Padding(
              padding: const EdgeInsets.only(left: 52),
              child: Text(
                meta,
                style: const TextStyle(
                  fontFamily: 'JetBrainsMono',
                  fontSize: 10,
                  color: CwColors.ink3,
                ),
              ),
            ),
            // Action button (recovery card only)
            if (actionLabel != null) ...[
              const SizedBox(height: 10),
              SizedBox(
                width: double.infinity,
                child: FilledButton(
                  onPressed: _isAuthenticated
                      ? () {}
                      : () => _authenticate(),
                  style: FilledButton.styleFrom(
                    backgroundColor: CwColors.accent,
                    foregroundColor: Colors.white,
                    minimumSize: const Size.fromHeight(40),
                    shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(10),
                    ),
                    textStyle: const TextStyle(
                      fontSize: 13,
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                  child: Text(_isAuthenticated ? actionLabel : S.biometricAuthReason),
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}
