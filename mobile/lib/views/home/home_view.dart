import 'package:flutter/material.dart';
import '../../theme/colors.dart';
import '../../l10n/strings.dart';
import '../../widgets/section_label.dart';
import '../../state/app_state.dart';
import '../../main.dart';
import '../../services/locator.dart';
import '../../router/app_router.dart';
import '../../api/tx_history_api.dart';

class HomeView extends StatefulWidget {
  const HomeView({super.key});

  @override
  State<HomeView> createState() => _HomeViewState();
}

class _HomeViewState extends State<HomeView> {
  List<Map<String, dynamic>> _transactions = [];
  bool _txLoading = true;
  String? _txError;
  int _lastChainId = 0;
  late final AppState _appState;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _appState = CowalletApp.of(context);
      _lastChainId = _appState.selectedChain.chainId;
      _appState.addListener(_onAppStateChange);
      _fetchTransactions();
    });
  }

  @override
  void dispose() {
    _appState.removeListener(_onAppStateChange);
    super.dispose();
  }

  void _onAppStateChange() {
    final currentChainId = _appState.selectedChain.chainId;
    if (_lastChainId != currentChainId) {
      _lastChainId = currentChainId;
      _fetchTransactions();
    }
  }

  Future<void> _fetchTransactions() async {
    final address = _appState.walletAddress;
    if (address.isEmpty) {
      setState(() {
        _txLoading = false;
        _transactions = [];
      });
      return;
    }

    setState(() {
      _txLoading = true;
      _txError = null;
    });

    try {
      final result = await TxHistoryApi.getHistory(
        address: address,
        chainId: _appState.selectedChain.chainId,
      );
      if (result.isSuccess && result.data != null) {
        final data = result.data!;
        final txList = data['transactions'] as List<dynamic>? ?? [];
        setState(() {
          _transactions = txList
              .map((item) => item as Map<String, dynamic>)
              .take(5)
              .toList();
          _txLoading = false;
        });
      } else {
        setState(() {
          _txError = result.errorMessage;
          _txLoading = false;
        });
      }
    } catch (e) {
      setState(() {
        _txError = e.toString();
        _txLoading = false;
      });
    }
  }

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
            Row(
              children: [
                Text(
                  '📊',
                  style: const TextStyle(fontSize: 18),
                ),
                const SizedBox(width: 8),
                Text(
                  S.yourTotal,
                  style: tt.bodySmall,
                ),
              ],
            ),
            const SizedBox(height: 12),
            Text(
              bal.loading ? '...' : bal.formattedTotal,
              style: tt.displayLarge?.copyWith(
                fontSize: 36,
                fontWeight: FontWeight.w700,
                fontFeatures: const [FontFeature.tabularFigures()],
              ),
            ),
            if (bal.error != null) ...[
              const SizedBox(height: 8),
              Text(
                bal.error!,
                style: tt.bodySmall?.copyWith(color: CwColors.danger),
              ),
            ] else if (!bal.loading && bal.tokens.isNotEmpty) ...[
              const SizedBox(height: 16),
              const Divider(height: 1),
              const SizedBox(height: 12),
              ...bal.topTokens.map((token) => _tokenRow(context, token)),
            ],
          ],
        ),
      ),
    );
  }

  Widget _tokenRow(BuildContext context, token) {
    final symbol = token.symbol as String;
    final balance = token.balance as String;
    final usd = token.usd as String;

    String emoji = '🪙';
    if (symbol == 'ETH') emoji = 'Ⓔ';
    if (symbol == 'USDC') emoji = 'Ⓤ';
    if (symbol == 'USDT') emoji = 'Ⓣ';

    return Padding(
      padding: const EdgeInsets.only(bottom: 8),
      child: Row(
        children: [
          Text(
            emoji,
            style: const TextStyle(fontSize: 16),
          ),
          const SizedBox(width: 10),
          Text(
            symbol,
            style: Theme.of(context).textTheme.titleMedium?.copyWith(
              fontSize: 13,
              fontWeight: FontWeight.w600,
            ),
          ),
          const Spacer(),
          Column(
            crossAxisAlignment: CrossAxisAlignment.end,
            children: [
              Text(
                balance,
                style: const TextStyle(
                  fontFamily: 'JetBrainsMono',
                  fontSize: 13,
                  fontWeight: FontWeight.w600,
                  color: CwColors.ink1,
                ),
              ),
              Text(
                '\$$usd',
                style: const TextStyle(
                  fontFamily: 'JetBrainsMono',
                  fontSize: 11,
                  color: CwColors.ink3,
                ),
              ),
            ],
          ),
        ],
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
              CwColors.accentSoft, () => AppShell.goToChatAndSend(context, S.actionSend)),
          _actionBtn(
              context,
              Icons.arrow_downward,
              S.receive,
              CwColors.success,
              CwColors.successSoft,
              () => AppShell.goToChatAndSend(context, S.actionReceive)),
          _actionBtn(context, Icons.qr_code_scanner, S.scan, CwColors.info,
              CwColors.infoSoft, () => Navigator.of(context).pushNamed(AppRouter.scan)),
          _actionBtn(context, Icons.people_outline, S.people, CwColors.gold,
              CwColors.goldSoft, () => Navigator.of(context).pushNamed(AppRouter.contacts)),
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
          _tryCard(context, '💬', S.try1h, S.try1s, S.try1h),
          const SizedBox(height: 10),
          _tryCard(context, '💰', S.try2h, S.try2s, S.try2h),
          const SizedBox(height: 10),
          _tryCard(context, '🎁', S.try3h, S.try3s, S.try3h),
        ],
      ),
    );
  }

  Widget _tryCard(
      BuildContext context, String emoji, String title, String subtitle, String chatMessage) {
    return GestureDetector(
      onTap: () => AppShell.goToChatAndSend(context, chatMessage),
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
          _buildActivityContent(context),
        ],
      ),
    );
  }

  Widget _buildActivityContent(BuildContext context) {
    if (_txLoading) {
      return Container(
        padding: const EdgeInsets.all(32),
        decoration: BoxDecoration(
          color: CwColors.bgCard,
          borderRadius: BorderRadius.circular(14),
          border: Border.all(color: CwColors.line),
        ),
        child: const Center(
          child: SizedBox(
            width: 20,
            height: 20,
            child: CircularProgressIndicator(strokeWidth: 2),
          ),
        ),
      );
    }

    if (_txError != null || _transactions.isEmpty) {
      return Container(
        padding: const EdgeInsets.all(32),
        decoration: BoxDecoration(
          color: CwColors.bgCard,
          borderRadius: BorderRadius.circular(14),
          border: Border.all(color: CwColors.line),
        ),
        child: Center(
          child: Text(
            S.noTxYet,
            style: Theme.of(context).textTheme.bodySmall,
          ),
        ),
      );
    }

    final walletAddress = CowalletApp.of(context).walletAddress.toLowerCase();

    return Container(
      decoration: BoxDecoration(
        color: CwColors.bgCard,
        borderRadius: BorderRadius.circular(14),
        border: Border.all(color: CwColors.line),
      ),
      child: Column(
        children: [
          for (int i = 0; i < _transactions.length; i++) ...[
            if (i > 0) const Divider(indent: 56, height: 1),
            _buildTxRow(context, _transactions[i], walletAddress),
          ],
        ],
      ),
    );
  }

  Widget _buildTxRow(
      BuildContext context, Map<String, dynamic> tx, String walletAddress) {
    final from = (tx['from'] as String? ?? '').toLowerCase();
    final to = (tx['to'] as String? ?? '').toLowerCase();
    final isReceive = to == walletAddress;
    final value = tx['value'] as String? ?? '0';
    final timestamp = tx['timestamp'] as String? ?? '';
    final status = tx['status'] as String? ?? '';
    final tokenSymbol = tx['token_symbol'] as String? ?? 'ETH';

    // Format value from wei to ETH (18 decimals)
    final formattedValue = _formatWeiValue(value);

    // Direction icon and colors
    final IconData icon;
    final Color iconColor;
    final Color iconBg;
    if (status == 'failed') {
      icon = Icons.close;
      iconColor = CwColors.danger;
      iconBg = CwColors.dangerSoft;
    } else if (isReceive) {
      icon = Icons.arrow_downward;
      iconColor = CwColors.success;
      iconBg = CwColors.successSoft;
    } else {
      icon = Icons.arrow_upward;
      iconColor = CwColors.accent;
      iconBg = CwColors.accentSoft;
    }

    // Format the address preview
    final peerAddress = isReceive ? from : to;
    final addressPreview = peerAddress.length >= 10
        ? '${peerAddress.substring(0, 6)}...${peerAddress.substring(peerAddress.length - 4)}'
        : peerAddress;

    // Title
    final title = isReceive
        ? '${S.receive} $formattedValue $tokenSymbol'
        : '${S.send} $formattedValue $tokenSymbol';

    // Subtitle with relative time and address
    final relativeTime = _formatRelativeTime(timestamp);
    final subtitle = '$addressPreview · $relativeTime';

    // Trailing amount
    final trailingText = isReceive
        ? '+$formattedValue $tokenSymbol'
        : '-$formattedValue $tokenSymbol';
    final trailingColor = isReceive ? CwColors.success : CwColors.ink2;

    return _activityRow(
      context,
      icon: icon,
      iconColor: iconColor,
      iconBg: iconBg,
      title: title,
      subtitle: subtitle,
      trailing: formattedValue != '0' ? trailingText : null,
      trailingColor: trailingColor,
    );
  }

  String _formatWeiValue(String weiValue) {
    if (weiValue == '0' || weiValue.isEmpty) return '0';
    try {
      final wei = BigInt.tryParse(weiValue);
      if (wei == null || wei == BigInt.zero) return '0';
      final ethValue = wei / BigInt.from(10).pow(18);
      final remainder = wei % BigInt.from(10).pow(18);
      if (remainder == BigInt.zero) return ethValue.toString();
      // Show up to 4 decimal places
      final fracStr = remainder.toString().padLeft(18, '0');
      final trimmed = fracStr.substring(0, 4).replaceAll(RegExp(r'0+$'), '');
      if (trimmed.isEmpty) return ethValue.toString();
      return '$ethValue.$trimmed';
    } catch (_) {
      return '0';
    }
  }

  String _formatRelativeTime(String isoTimestamp) {
    if (isoTimestamp.isEmpty) return '';
    try {
      final dt = DateTime.parse(isoTimestamp);
      final now = DateTime.now();
      final diff = now.difference(dt);
      if (diff.inMinutes < 1) return S.justNow;
      if (diff.inMinutes < 60) return S.minutesAgo(diff.inMinutes);
      if (diff.inHours < 24) return S.hoursAgo(diff.inHours);
      return S.daysAgo(diff.inDays);
    } catch (_) {
      return '';
    }
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
