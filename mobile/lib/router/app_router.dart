import 'package:flutter/material.dart';
import '../views/home/home_view.dart';
import '../views/wallet/wallet_view.dart';
import '../views/chat/chat_view.dart';
import '../views/yield/defi_hub_view.dart';
import '../views/settings/settings_view.dart';
import '../views/keys/keys_view.dart';
import '../views/send/send_view.dart';
import '../views/receive/receive_view.dart';
import '../views/scan/scan_view.dart';
import '../views/contacts/contacts_view.dart';
import '../onboarding/onboarding_flow.dart';
import '../theme/colors.dart';
import '../l10n/strings.dart';

class AppRouter {
  static const home = '/';
  static const onboarding = '/onboarding';
  static const keys = '/keys';
  static const send = '/send';
  static const receive = '/receive';
  static const scan = '/scan';
  static const contacts = '/contacts';

  static Route<dynamic> onGenerateRoute(RouteSettings settings) {
    switch (settings.name) {
      case onboarding:
        return MaterialPageRoute(builder: (_) => const OnboardingFlow());
      case keys:
        return MaterialPageRoute(builder: (_) => const KeysView());
      case send:
        return MaterialPageRoute(builder: (_) => const SendView());
      case receive:
        return MaterialPageRoute(builder: (_) => const ReceiveView());
      case scan:
        return MaterialPageRoute(builder: (_) => const ScanView());
      case contacts:
        return MaterialPageRoute(builder: (_) => const ContactsView());
      default:
        return MaterialPageRoute(builder: (_) => const AppShell());
    }
  }
}

class AppShell extends StatefulWidget {
  const AppShell({super.key});

  static final chatKey = GlobalKey<ChatViewState>();

  static void goToChatAndSend(BuildContext context, String message) {
    final shellState = context.findAncestorStateOfType<_AppShellState>();
    if (shellState != null) {
      shellState.switchToChat();
      WidgetsBinding.instance.addPostFrameCallback((_) {
        chatKey.currentState?.sendMessage(message);
      });
    }
  }

  static void goToChatAndShowTx(BuildContext context, Map<String, dynamic> txData) {
    final shellState = context.findAncestorStateOfType<_AppShellState>();
    if (shellState != null) {
      shellState.switchToChat();
      WidgetsBinding.instance.addPostFrameCallback((_) {
        chatKey.currentState?.showTxDetail(txData);
      });
    }
  }

  @override
  State<AppShell> createState() => _AppShellState();
}

class _AppShellState extends State<AppShell> {
  int _currentIndex = 0;

  void switchToChat() {
    setState(() => _currentIndex = 2);
  }

  final _views = <Widget>[
    const HomeView(),
    const WalletView(),
    ChatView(key: AppShell.chatKey),
    const DefiHubView(),
    const SettingsView(),
  ];

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: IndexedStack(index: _currentIndex, children: _views),
      bottomNavigationBar: _buildTabBar(),
    );
  }

  Widget _buildTabBar() {
    return Container(
      decoration: const BoxDecoration(
        color: CwColors.bgPaper,
        border: Border(top: BorderSide(color: CwColors.line)),
      ),
      child: SafeArea(
        child: SizedBox(
          height: 56,
          child: Row(
            children: [
              _tabItem(0, Icons.home_outlined, Icons.home, S.tabHome),
              _tabItem(1, Icons.account_balance_wallet_outlined,
                  Icons.account_balance_wallet, S.tabWallet),
              _askPill(),
              _tabItem(3, Icons.trending_up_outlined, Icons.trending_up, S.tabDefi),
              _tabItem(4, Icons.settings_outlined, Icons.settings, S.tabSettings),
            ],
          ),
        ),
      ),
    );
  }

  Widget _tabItem(int index, IconData icon, IconData activeIcon, String label) {
    final active = _currentIndex == index;
    final color = active ? CwColors.accent : CwColors.ink4;
    return Expanded(
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: () => setState(() => _currentIndex = index),
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Icon(active ? activeIcon : icon, size: 22, color: color),
            const SizedBox(height: 2),
            Text(label, style: TextStyle(fontSize: 10, color: color)),
          ],
        ),
      ),
    );
  }

  Widget _askPill() {
    return Expanded(
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: () => setState(() => _currentIndex = 2),
        child: Center(
          child: Transform.translate(
            offset: const Offset(0, -12),
            child: Container(
              width: 56,
              height: 56,
              decoration: BoxDecoration(
                shape: BoxShape.circle,
                color: CwColors.ink1,
                boxShadow: [
                  BoxShadow(
                    color: CwColors.ink1.withValues(alpha: 0.3),
                    blurRadius: 8,
                    offset: const Offset(0, 2),
                  ),
                ],
              ),
              child: Center(
                child: Container(
                  width: 28,
                  height: 28,
                  decoration: const BoxDecoration(
                    shape: BoxShape.circle,
                    gradient: RadialGradient(
                      center: Alignment(-0.2, -0.2),
                      colors: [Color(0xFFFFBFA6), Color(0xFFD97757), Color(0xFFB86A4A)],
                      stops: [0.0, 0.5, 1.0],
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
