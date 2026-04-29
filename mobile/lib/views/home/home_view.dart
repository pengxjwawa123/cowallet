import 'package:flutter/material.dart';
import '../../theme/colors.dart';
import '../../l10n/strings.dart';
import '../../widgets/section_label.dart';
import '../../main.dart';
import '../../services/locator.dart';

class HomeView extends StatelessWidget {
  const HomeView({super.key});

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      child: CustomScrollView(
        slivers: [
          SliverToBoxAdapter(child: _appHeader(context)),
          SliverToBoxAdapter(child: _statusBar(context)),
          SliverToBoxAdapter(child: _greeting(context)),
          SliverToBoxAdapter(child: _slogan(context)),
          SliverToBoxAdapter(child: _balanceCard(context)),
          SliverToBoxAdapter(child: _actionButtons(context)),
          SliverToBoxAdapter(child: _tryTalkingSection(context)),
          SliverToBoxAdapter(child: _recentActivitySection(context)),
          SliverToBoxAdapter(child: _showcaseSection(context)),
          const SliverToBoxAdapter(child: SizedBox(height: 32)),
        ],
      ),
    );
  }

  // ── 1. App header ──────────────────────────────────────────────────────

  Widget _appHeader(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(20, 14, 12, 0),
      child: Row(
        children: [
          Container(
            width: 10,
            height: 10,
            decoration: const BoxDecoration(
              color: CwColors.accent,
              shape: BoxShape.circle,
            ),
          ),
          const SizedBox(width: 8),
          Text(
            S.appName,
            style: Theme.of(context).textTheme.titleLarge,
          ),
          const Spacer(),
          IconButton(
            icon: const Icon(Icons.search, color: CwColors.ink3, size: 22),
            onPressed: () {},
          ),
          IconButton(
            icon: const Icon(Icons.menu, color: CwColors.ink3, size: 22),
            onPressed: () {},
          ),
        ],
      ),
    );
  }

  // ── 2. Status bar ──────────────────────────────────────────────────────

  Widget _statusBar(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(20, 14, 20, 0),
      child: Row(
        children: [
          Container(
            width: 7,
            height: 7,
            decoration: const BoxDecoration(
              color: CwColors.success,
              shape: BoxShape.circle,
            ),
          ),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              S.homeStatus,
              style: Theme.of(context).textTheme.bodySmall,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
            ),
          ),
        ],
      ),
    );
  }

  // ── 3. Greeting ────────────────────────────────────────────────────────

  Widget _greeting(BuildContext context) {
    final appState = CowalletApp.of(context);
    final name = appState.userName;
    final displayName = name.isEmpty ? 'Alice' : name;
    final address = appState.walletAddress;
    return Padding(
      padding: const EdgeInsets.fromLTRB(20, 20, 20, 0),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          RichText(
            text: TextSpan(
              style: Theme.of(context).textTheme.displayLarge,
              children: [
                TextSpan(text: S.homeGreetMorning),
                TextSpan(
                  text: '$displayName。',
                  style: const TextStyle(fontStyle: FontStyle.italic),
                ),
              ],
            ),
          ),
          if (address.isNotEmpty) ...[
            const SizedBox(height: 4),
            Text(
              '${address.substring(0, 6)}...${address.substring(address.length - 4)}',
              style: Theme.of(context).textTheme.bodySmall?.copyWith(
                    fontFamily: 'JetBrainsMono',
                    color: CwColors.ink4,
                    fontSize: 11,
                  ),
            ),
          ],
        ],
      ),
    );
  }

  // ── 4. Slogan ──────────────────────────────────────────────────────────

  Widget _slogan(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(20, 8, 20, 0),
      child: Text(
        S.homeSlogan,
        style: Theme.of(context).textTheme.bodyMedium,
      ),
    );
  }

  // ── 5. Balance card ────────────────────────────────────────────────────

  Widget _balanceCard(BuildContext context) {
    final tt = Theme.of(context).textTheme;
    final bal = Services.balance;
    return ListenableBuilder(
      listenable: bal,
      builder: (context, _) => Container(
        margin: const EdgeInsets.fromLTRB(20, 20, 20, 0),
        padding: const EdgeInsets.all(24),
        decoration: BoxDecoration(
          color: CwColors.bgCard,
          borderRadius: BorderRadius.circular(20),
          border: Border.all(color: CwColors.line),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              S.yourTotal,
              style: tt.bodySmall,
            ),
            const SizedBox(height: 10),
            Text(
              bal.loading ? '...' : bal.formattedTotal,
              style: tt.displayLarge?.copyWith(
                fontSize: 34,
                fontFeatures: const [FontFeature.tabularFigures()],
              ),
            ),
            const SizedBox(height: 6),
            if (bal.error != null)
              Text(
                bal.error!,
                style: tt.bodyMedium?.copyWith(color: CwColors.danger),
              )
            else
              Row(
                children: [
                  Text(
                    bal.loading ? '...' : bal.formattedTotal,
                    style: tt.bodyMedium?.copyWith(color: CwColors.success),
                  ),
                  const SizedBox(width: 8),
                  Text(
                    S.today,
                    style: tt.bodySmall?.copyWith(color: CwColors.ink4),
                  ),
                ],
              ),
          ],
        ),
      ),
    );
  }

  // ── 6. Action buttons ─────────────────────────────────────────────────

  Widget _actionButtons(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(20, 18, 20, 0),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: [
          _actionBtn(context, Icons.arrow_upward, S.send, CwColors.accent,
              CwColors.accentSoft, () => Navigator.pushNamed(context, '/send')),
          _actionBtn(
              context,
              Icons.arrow_downward,
              S.receive,
              CwColors.success,
              CwColors.successSoft,
              () => Navigator.pushNamed(context, '/receive')),
          _actionBtn(context, Icons.qr_code_scanner, S.scan, CwColors.info,
              CwColors.infoSoft, () {}),
          _actionBtn(context, Icons.people_outline, S.people, CwColors.gold,
              CwColors.goldSoft, () {}),
        ],
      ),
    );
  }

  Widget _actionBtn(BuildContext context, IconData icon, String label,
      Color color, Color bgColor, VoidCallback onTap) {
    return GestureDetector(
      onTap: onTap,
      child: SizedBox(
        width: 68,
        child: Column(
          children: [
            Container(
              width: 56,
              height: 56,
              decoration: BoxDecoration(
                color: bgColor,
                shape: BoxShape.circle,
              ),
              child: Icon(icon, color: color, size: 24),
            ),
            const SizedBox(height: 8),
            Text(
              label,
              style: TextStyle(
                fontSize: 12,
                color: color,
                fontWeight: FontWeight.w500,
              ),
            ),
          ],
        ),
      ),
    );
  }

  // ── 7. Try talking section ─────────────────────────────────────────────

  Widget _tryTalkingSection(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 20),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SectionLabel(title: S.tryTalking),
          _tryCard(context, '💬', S.try1h, S.try1s),
          const SizedBox(height: 10),
          _tryCard(context, '💰', S.try2h, S.try2s),
          const SizedBox(height: 10),
          _tryCard(context, '🎁', S.try3h, S.try3s),
        ],
      ),
    );
  }

  Widget _tryCard(
      BuildContext context, String emoji, String title, String subtitle) {
    return GestureDetector(
      onTap: () {},
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 14),
        decoration: BoxDecoration(
          color: CwColors.bgCard,
          borderRadius: BorderRadius.circular(14),
          border: Border.all(color: CwColors.line),
        ),
        child: Row(
          children: [
            Text(emoji, style: const TextStyle(fontSize: 24)),
            const SizedBox(width: 14),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    title,
                    style: Theme.of(context).textTheme.titleMedium?.copyWith(
                          color: CwColors.ink1,
                        ),
                  ),
                  const SizedBox(height: 2),
                  Text(
                    subtitle,
                    style: Theme.of(context).textTheme.bodySmall,
                  ),
                ],
              ),
            ),
            const SizedBox(width: 8),
            const Icon(Icons.chevron_right, color: CwColors.ink4, size: 20),
          ],
        ),
      ),
    );
  }

  // ── 8. Recent activity section ─────────────────────────────────────────

  Widget _recentActivitySection(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 20),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SectionLabel(
            title: S.recentActivity,
            trailing: S.seeAll,
            onTrailingTap: () {},
          ),
          Container(
            decoration: BoxDecoration(
              color: CwColors.bgCard,
              borderRadius: BorderRadius.circular(14),
              border: Border.all(color: CwColors.line),
            ),
            child: Column(
              children: [
                _activityRow(
                  context,
                  icon: Icons.arrow_downward,
                  iconColor: CwColors.success,
                  iconBg: CwColors.successSoft,
                  title: S.actRecv,
                  subtitle: S.actRecvSub,
                  trailing: '+0.5 ETH',
                  trailingColor: CwColors.success,
                ),
                const Divider(indent: 56, height: 1),
                _activityRow(
                  context,
                  icon: Icons.visibility_outlined,
                  iconColor: CwColors.info,
                  iconBg: CwColors.infoSoft,
                  title: S.actAi,
                  subtitle: S.actAiSub,
                ),
                const Divider(indent: 56, height: 1),
                _activityRow(
                  context,
                  icon: Icons.autorenew,
                  iconColor: CwColors.accent,
                  iconBg: CwColors.accentSoft,
                  title: S.actPay,
                  subtitle: S.actPaySub,
                  trailing: '-\$42',
                  trailingColor: CwColors.ink2,
                ),
                const Divider(indent: 56, height: 1),
                _activityRow(
                  context,
                  icon: Icons.shield_outlined,
                  iconColor: CwColors.danger,
                  iconBg: CwColors.dangerSoft,
                  title: S.actBlock,
                  subtitle: S.actBlockSub,
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _activityRow(
    BuildContext context, {
    required IconData icon,
    required Color iconColor,
    required Color iconBg,
    required String title,
    required String subtitle,
    String? trailing,
    Color? trailingColor,
  }) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
      child: Row(
        children: [
          Container(
            width: 36,
            height: 36,
            decoration: BoxDecoration(
              color: iconBg,
              borderRadius: BorderRadius.circular(10),
            ),
            child: Icon(icon, color: iconColor, size: 18),
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  title,
                  style: Theme.of(context).textTheme.titleMedium?.copyWith(
                        color: CwColors.ink1,
                        fontSize: 14,
                      ),
                ),
                const SizedBox(height: 2),
                Text(
                  subtitle,
                  style: Theme.of(context).textTheme.bodySmall?.copyWith(
                        fontSize: 11,
                      ),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ],
            ),
          ),
          if (trailing != null) ...[
            const SizedBox(width: 8),
            Text(
              trailing,
              style: Theme.of(context).textTheme.labelLarge?.copyWith(
                    color: trailingColor ?? CwColors.ink2,
                    fontSize: 13,
                  ),
            ),
          ],
        ],
      ),
    );
  }

  // ── 9. Showcase section ────────────────────────────────────────────────

  Widget _showcaseSection(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 20),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SectionLabel(title: S.onlyCowallet),
          Row(
            children: [
              Expanded(
                child: _showcaseCard(
                  context,
                  gradientColors: [CwColors.accent, CwColors.accentHover],
                  icon: Icons.auto_awesome,
                  title: S.scAiReads,
                  desc: S.scAiReadsSub,
                ),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: _showcaseCard(
                  context,
                  gradientColors: [CwColors.success, const Color(0xFF3D6B3E)],
                  icon: Icons.smart_toy_outlined,
                  title: S.scAgentPay,
                  desc: S.scAgentPaySub,
                ),
              ),
            ],
          ),
          const SizedBox(height: 12),
          Row(
            children: [
              Expanded(
                child: _showcaseCard(
                  context,
                  gradientColors: [CwColors.gold, const Color(0xFF8A6A2A)],
                  icon: Icons.family_restroom,
                  title: S.scFamily,
                  desc: S.scFamilySub,
                ),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: _showcaseCard(
                  context,
                  gradientColors: [CwColors.info, const Color(0xFF2A4F6E)],
                  icon: Icons.extension_outlined,
                  title: S.scSkills,
                  desc: S.scSkillsSub,
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }

  Widget _showcaseCard(
    BuildContext context, {
    required List<Color> gradientColors,
    required IconData icon,
    required String title,
    required String desc,
  }) {
    return Container(
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: CwColors.bgCard,
        borderRadius: BorderRadius.circular(14),
        border: Border.all(color: CwColors.line),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Container(
            width: 36,
            height: 36,
            decoration: BoxDecoration(
              borderRadius: BorderRadius.circular(10),
              gradient: LinearGradient(
                begin: Alignment.topLeft,
                end: Alignment.bottomRight,
                colors: gradientColors,
              ),
            ),
            child: Icon(icon, color: Colors.white, size: 20),
          ),
          const SizedBox(height: 12),
          Text(
            title,
            style: Theme.of(context).textTheme.titleMedium?.copyWith(
                  color: CwColors.ink1,
                  fontSize: 14,
                ),
          ),
          const SizedBox(height: 4),
          Text(
            desc,
            style: Theme.of(context).textTheme.bodySmall?.copyWith(
                  fontSize: 11,
                  height: 1.4,
                ),
          ),
        ],
      ),
    );
  }
}
