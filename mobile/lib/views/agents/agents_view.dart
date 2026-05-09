import 'package:flutter/material.dart';
import '../../theme/colors.dart';
import '../../l10n/strings.dart';
import '../../widgets/section_label.dart';
import '../../widgets/cw_chip.dart';

class AgentsView extends StatelessWidget {
  const AgentsView({super.key});

  @override
  Widget build(BuildContext context) {
    final tt = Theme.of(context).textTheme;

    return ListView(
      padding: const EdgeInsets.symmetric(horizontal: 20),
      children: [
        const SizedBox(height: 12),

          // ── Hero card ──
          _heroCard(tt),
          const SizedBox(height: 8),

          // ── Section: 已接入 · 2 个助手 ──
          SectionLabel(
            title: S.connected,
            trailing: S.freezeAll,
            trailingColor: CwColors.danger,
            onTrailingTap: () {},
          ),
          _agentCard(
            context,
            initial: 'C',
            bgColor: CwColors.ink1,
            name: 'Claude Desktop',
            rules: '只读余额、每日上限 \$500、不能碰质押',
            active: true,
            usageAmount: 42,
            usageLimit: 500,
            meta: '已签 3/10 笔',
          ),
          const SizedBox(height: 8),
          _agentCard(
            context,
            initial: 'o',
            bgColor: CwColors.accent,
            name: 'Cowork',
            rules: '团队报销、审批后自动付款',
            active: true,
            usageAmount: 0,
            usageLimit: 1000,
            meta: '',
          ),
          const SizedBox(height: 14),

          // ── Connect new agent button ──
          SizedBox(
            height: 48,
            child: OutlinedButton.icon(
              onPressed: () {},
              icon: const Icon(Icons.add, size: 18),
              label: Text(S.connectNew),
              style: OutlinedButton.styleFrom(
                foregroundColor: CwColors.ink1,
                side: const BorderSide(color: CwColors.lineStrong),
                shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(14),
                ),
              ),
            ),
          ),

          // ── Section: Skills ──
          SectionLabel(title: S.skillsLabel),
          Text(
            S.skillsIntro,
            style: tt.bodyMedium,
          ),
          const SizedBox(height: 10),
          _skillsGrid(),

          // ── Section: 开发者接口 ──
          SectionLabel(
            title: S.devProtocols,
            trailingWidget: const CwChip(
              label: 'for devs',
              variant: ChipVariant.info,
              fontSize: 10,
            ),
          ),
          _protocolList(context),

        const SizedBox(height: 32),
      ],
    );
  }

  // ── Hero card ───────────────────────────────────────────────────────────

  Widget _heroCard(TextTheme tt) {
    return Container(
      padding: const EdgeInsets.all(20),
      decoration: BoxDecoration(
        gradient: const LinearGradient(
          begin: Alignment.topLeft,
          end: Alignment.bottomRight,
          colors: [Color(0xFFF9EDE0), Color(0xFFF7E3D8)],
        ),
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: CwColors.line),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            S.agentsLabel,
            style: const TextStyle(
              fontFamily: 'JetBrainsMono',
              fontSize: 11,
              fontWeight: FontWeight.w600,
              letterSpacing: 0.5,
              color: CwColors.ink3,
            ),
          ),
          const SizedBox(height: 8),
          Text(
            S.agentsH1,
            style: tt.headlineLarge,
          ),
          const SizedBox(height: 6),
          Text(
            S.agentsSub,
            style: tt.bodyMedium?.copyWith(color: CwColors.ink3),
          ),
        ],
      ),
    );
  }

  // ── Agent card ──────────────────────────────────────────────────────────

  Widget _agentCard(
    BuildContext context, {
    required String initial,
    required Color bgColor,
    required String name,
    required String rules,
    required bool active,
    required double usageAmount,
    required double usageLimit,
    required String meta,
  }) {
    final pct = (usageAmount / usageLimit).clamp(0.0, 1.0);

    return Container(
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
        color: CwColors.bgCard,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: CwColors.line),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // Top row: avatar + name + chip
          Row(
            children: [
              // Square avatar with initial
              Container(
                width: 36,
                height: 36,
                decoration: BoxDecoration(
                  color: bgColor,
                  borderRadius: BorderRadius.circular(10),
                ),
                child: Center(
                  child: Text(
                    initial,
                    style: const TextStyle(
                      color: Colors.white,
                      fontWeight: FontWeight.w700,
                      fontSize: 16,
                    ),
                  ),
                ),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      name,
                      style: Theme.of(context).textTheme.titleMedium?.copyWith(
                            fontWeight: FontWeight.w600,
                            color: CwColors.ink1,
                          ),
                    ),
                    const SizedBox(height: 2),
                    Text(
                      rules,
                      style: Theme.of(context).textTheme.bodySmall,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                  ],
                ),
              ),
              const SizedBox(width: 8),
              if (active)
                CwChip(
                  label: S.active,
                  variant: ChipVariant.green,
                  showDot: true,
                  fontSize: 11,
                ),
            ],
          ),

          const SizedBox(height: 12),

          // Usage bar
          Row(
            children: [
              Text(
                '\$${usageAmount.toInt()} / \$${usageLimit.toInt()}',
                style: const TextStyle(
                  fontFamily: 'JetBrainsMono',
                  fontSize: 11,
                  fontWeight: FontWeight.w500,
                  color: CwColors.ink3,
                ),
              ),
              if (meta.isNotEmpty) ...[
                const Spacer(),
                Text(
                  meta,
                  style: const TextStyle(
                    fontFamily: 'Inter',
                    fontSize: 11,
                    color: CwColors.ink3,
                  ),
                ),
              ],
            ],
          ),
          const SizedBox(height: 6),
          ClipRRect(
            borderRadius: BorderRadius.circular(2),
            child: SizedBox(
              height: 4,
              child: Stack(
                children: [
                  Container(
                    decoration: BoxDecoration(
                      color: CwColors.bgSubtle,
                      borderRadius: BorderRadius.circular(2),
                    ),
                  ),
                  FractionallySizedBox(
                    widthFactor: pct,
                    child: Container(
                      decoration: BoxDecoration(
                        color: CwColors.accent,
                        borderRadius: BorderRadius.circular(2),
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }

  // ── Skills grid ─────────────────────────────────────────────────────────

  Widget _skillsGrid() {
    final skills = [
      _SkillItem('⚡', 'auto-pay', true),
      _SkillItem('🔔', 'price alerts', true),
      _SkillItem('🏠', 'rent', false),
      _SkillItem('📈', 'DCA', false),
      _SkillItem('👥', 'subaccount', false),
      _SkillItem('🔍', 'browse more', false),
    ];

    return GridView.builder(
      shrinkWrap: true,
      physics: const NeverScrollableScrollPhysics(),
      gridDelegate: const SliverGridDelegateWithFixedCrossAxisCount(
        crossAxisCount: 3,
        crossAxisSpacing: 8,
        mainAxisSpacing: 8,
        childAspectRatio: 1.0,
      ),
      itemCount: skills.length,
      itemBuilder: (context, i) {
        final s = skills[i];
        return Container(
          constraints: const BoxConstraints(minHeight: 108),
          padding: const EdgeInsets.all(10),
          decoration: BoxDecoration(
            color: CwColors.bgCard,
            borderRadius: BorderRadius.circular(14),
            border: Border.all(color: CwColors.line),
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                s.icon,
                style: const TextStyle(fontSize: 30),
              ),
              const Spacer(),
              Text(
                s.name,
                style: const TextStyle(
                  fontFamily: 'Inter',
                  fontSize: 12,
                  fontWeight: FontWeight.w600,
                  color: CwColors.ink1,
                ),
              ),
              const SizedBox(height: 4),
              if (s.installed)
                CwChip(
                  label: S.installed,
                  variant: ChipVariant.green,
                  fontSize: 10,
                )
              else
                CwChip(
                  label: S.addSkill,
                  variant: ChipVariant.accent,
                  fontSize: 10,
                ),
            ],
          ),
        );
      },
    );
  }

  // ── Protocol list ───────────────────────────────────────────────────────

  Widget _protocolList(BuildContext context) {
    final protocols = [
      _ProtocolItem('MCP', 'Model Context Protocol', true),
      _ProtocolItem('x402', 'HTTP 402 payments', true),
      _ProtocolItem('REST', 'REST API', true),
      _ProtocolItem('Webhook', 'Event notifications', true),
      _ProtocolItem('EIP-6963', 'Wallet discovery', true),
      _ProtocolItem('A2A', 'Agent-to-Agent', false),
    ];

    return Container(
      decoration: BoxDecoration(
        color: CwColors.bgCard,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: CwColors.line),
      ),
      child: Column(
        children: List.generate(protocols.length, (i) {
          final p = protocols[i];
          final isLast = i == protocols.length - 1;
          return Column(
            children: [
              Padding(
                padding:
                    const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
                child: Row(
                  children: [
                    // Protocol chip
                    Container(
                      padding: const EdgeInsets.symmetric(
                          horizontal: 8, vertical: 3),
                      decoration: BoxDecoration(
                        color: CwColors.bgSubtle,
                        borderRadius: BorderRadius.circular(6),
                      ),
                      child: Text(
                        p.name,
                        style: const TextStyle(
                          fontFamily: 'JetBrainsMono',
                          fontSize: 11,
                          fontWeight: FontWeight.w600,
                          color: CwColors.ink2,
                        ),
                      ),
                    ),
                    const SizedBox(width: 10),
                    Expanded(
                      child: Text(
                        p.description,
                        style: Theme.of(context).textTheme.bodySmall,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                    const SizedBox(width: 8),
                    CwChip(
                      label: p.live ? 'LIVE' : 'soon',
                      variant:
                          p.live ? ChipVariant.green : ChipVariant.amber,
                      fontSize: 10,
                    ),
                  ],
                ),
              ),
              if (!isLast)
                const Divider(height: 0, indent: 14, endIndent: 14),
            ],
          );
        }),
      ),
    );
  }
}

// ── Helper data classes ───────────────────────────────────────────────────

class _SkillItem {
  final String icon;
  final String name;
  final bool installed;
  const _SkillItem(this.icon, this.name, this.installed);
}

class _ProtocolItem {
  final String name;
  final String description;
  final bool live;
  const _ProtocolItem(this.name, this.description, this.live);
}
