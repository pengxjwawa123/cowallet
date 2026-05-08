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

      // Wallet exists locally — ensure we have a valid session
      final hasValidSession = await AuthApi.isLoggedIn();
      if (hasValidSession) {
        // Try to validate session; if expired, attempt refresh
        final sessionResult = await AuthApi.getSessionInfo();
        if (!sessionResult.isSuccess) {
          final refreshed = await AuthApi.refreshToken();
          if (!refreshed) {
            await _reloginWithDeviceId();
          }
        }
      } else {
        // No token — try refresh first, then re-login with device_id
        final refreshed = await AuthApi.refreshToken();
        if (!refreshed) {
          await _reloginWithDeviceId();
        }
      }

      // Check again after potential refresh/re-login
      final hasToken = await AuthApi.isLoggedIn();
      if (hasToken) {
        final addr = await Services.wallet.getAddress();
        appState.setWalletAddress(addr);
        appState.completeOnboarding();
        _initialRoute = AppRouter.home;
      }
    } catch (_) {
      // Fall through to onboarding
    }
    setState(() => _ready = true);
  }

  Future<void> _reloginWithDeviceId() async {
    final deviceId = await SecureStorage.getDeviceId();
    if (deviceId != null && deviceId.isNotEmpty) {
      await AuthApi.login(deviceId: deviceId);
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
