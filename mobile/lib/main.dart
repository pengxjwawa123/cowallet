import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'theme/theme.dart';
import 'router/app_router.dart';
import 'state/app_state.dart';
import 'services/locator.dart';
import 'api/auth_api.dart';

void main() async {
  WidgetsFlutterBinding.ensureInitialized();
  SystemChrome.setPreferredOrientations([
    DeviceOrientation.portraitUp,
  ]);
  await Services.init();
  runApp(const CowalletApp());
}

class CowalletApp extends StatefulWidget {
  const CowalletApp({super.key});

  static AppState of(BuildContext context) =>
      context.findAncestorStateOfType<_CowalletAppState>()!.appState;

  @override
  State<CowalletApp> createState() => _CowalletAppState();
}

class _CowalletAppState extends State<CowalletApp> {
  final appState = AppState();
  String _initialRoute = AppRouter.onboarding;
  bool _ready = false;

  @override
  void initState() {
    super.initState();
    _checkWalletState();
  }

  Future<void> _checkWalletState() async {
    try {
      // Check for both local wallet existence and valid backend session
      final hasLocalWallet = await Services.wallet.hasWallet();
      final hasValidSession = await AuthApi.isLoggedIn();

      if (hasLocalWallet && hasValidSession) {
        final addr = await Services.wallet.getAddress();
        appState.setWalletAddress(addr);
        appState.completeOnboarding();
        _initialRoute = AppRouter.home;

        // Try to refresh session info from backend
        try {
          await AuthApi.getSessionInfo();
        } catch (_) {}
      }
    } catch (_) {
      // Fall through to onboarding
    }
    setState(() => _ready = true);
  }

  @override
  void dispose() {
    appState.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (!_ready) {
      return MaterialApp(
        debugShowCheckedModeBanner: false,
        theme: cwTheme(),
        home: const Scaffold(body: SizedBox.shrink()),
      );
    }
    return ListenableBuilder(
      listenable: appState,
      builder: (context, _) => MaterialApp(
        title: 'cowallet',
        debugShowCheckedModeBanner: false,
        theme: cwTheme(),
        initialRoute: _initialRoute,
        onGenerateRoute: AppRouter.onGenerateRoute,
      ),
    );
  }
}
