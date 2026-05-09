import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'theme/theme.dart';
import 'router/app_router.dart';
import 'state/app_state.dart';
import 'services/locator.dart';
import 'api/auth_api.dart';
import 'utils/secure_storage.dart';

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
      final hasLocalWallet = await Services.wallet.hasWallet();
      print('[App] hasLocalWallet=$hasLocalWallet');
      if (!hasLocalWallet) {
        setState(() => _ready = true);
        return;
      }

      // Wallet exists locally — go to home immediately
      final addr = await Services.wallet.getAddress();
      appState.setWalletAddress(addr);
      appState.completeOnboarding();
      _initialRoute = AppRouter.home;
      setState(() => _ready = true);

      // Refresh session and balance in background (non-blocking)
      _refreshSessionInBackground();
      _refreshBalanceInBackground(addr);
    } catch (_) {
      setState(() => _ready = true);
    }
  }

  Future<void> _refreshSessionInBackground() async {
    try {
      final hasValidSession = await AuthApi.isLoggedIn();
      if (hasValidSession) {
        final sessionResult = await AuthApi.getSessionInfo();
        if (!sessionResult.isSuccess) {
          final refreshed = await AuthApi.refreshToken();
          if (!refreshed) await _reloginWithDeviceId();
        }
      } else {
        final refreshed = await AuthApi.refreshToken();
        if (!refreshed) await _reloginWithDeviceId();
      }
    } catch (_) {}
  }

  Future<void> _reloginWithDeviceId() async {
    final deviceId = await SecureStorage.getDeviceId();
    if (deviceId != null && deviceId.isNotEmpty) {
      await AuthApi.login(deviceId: deviceId);
    }
  }

  Future<void> _refreshBalanceInBackground(String address) async {
    try {
      await Services.balance.refresh(address, chainId: appState.selectedChain.chainId);
    } catch (_) {
      // Silently fail - balance will show error state in UI
    }
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
