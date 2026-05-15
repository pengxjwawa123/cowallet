import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'theme/theme.dart';
import 'router/app_router.dart';
import 'state/app_state.dart';
import 'services/locator.dart';
import 'services/push_service.dart';
import 'api/auth_api.dart';
import 'api/chains_api.dart';
import 'config/api_config.dart';
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

  // Global navigator key for push notification navigation
  final _navigatorKey = GlobalKey<NavigatorState>();

  @override
  void initState() {
    super.initState();
    _setupPushNotificationHandlers();
    _checkWalletState();
  }

  void _setupPushNotificationHandlers() {
    Services.push.onNotificationTap = _handlePushNotificationTap;
  }

  void _handlePushNotificationTap(Map<String, dynamic> data) {
    final type = data['type'] as String?;
    final context = _navigatorKey.currentContext;
    if (context == null) return;

    switch (type) {
      case PushType.txConfirmed:
      case PushType.txFailed:
        // Navigate to home and show tx detail in chat
        final txHash = data['tx_hash'] as String?;
        if (txHash != null) {
          _navigatorKey.currentState?.pushNamedAndRemoveUntil(
            AppRouter.home,
            (route) => false,
          );
        }
        break;
      case PushType.securityAlert:
        // Navigate to settings/security section
        _navigatorKey.currentState?.pushNamedAndRemoveUntil(
          AppRouter.home,
          (route) => false,
        );
        break;
      case PushType.mpcSignRequest:
        // Navigate to home (approval will be handled via the stream listener)
        _navigatorKey.currentState?.pushNamedAndRemoveUntil(
          AppRouter.home,
          (route) => false,
        );
        break;
    }
  }

  Future<void> _checkWalletState() async {
    try {
      _loadSupportedChains();
      appState.loadUserName();

      final hasLocalWallet = await Services.wallet.hasWallet();
      print('[App] hasLocalWallet=$hasLocalWallet');
      if (!hasLocalWallet) {
        setState(() => _ready = true);
        return;
      }

      // Wallet exists locally
      final addr = await Services.wallet.getAddress();
      appState.setWalletAddress(addr);
      appState.completeOnboarding();

      // Check if onboarding was interrupted mid-flow
      final savedStep = await SecureStorage.get(SecureStorage.keyOnboardingStep);
      if (savedStep != null && savedStep.isNotEmpty) {
        _initialRoute = AppRouter.onboarding;
        setState(() => _ready = true);
        return;
      }

      _initialRoute = AppRouter.home;

      // Ensure valid token before rendering home (prevents 401 cascades)
      await _refreshSessionInBackground();

      // Re-register push token now that auth is available
      Services.push.reregisterToken();

      // Start presign pool auto-refill monitoring
      Services.presignPool.start();

      setState(() => _ready = true);
      _refreshBalanceInBackground(addr);
    } catch (_) {
      setState(() => _ready = true);
    }
  }

  Future<void> _refreshSessionInBackground() async {
    try {
      final tokenValid = await AuthApi.isLoggedIn();
      if (tokenValid) return;

      // Token 过期或不存在，尝试 refresh
      final refreshed = await AuthApi.refreshToken();
      if (!refreshed) {
        await _reloginWithDeviceId();
      }
    } catch (_) {}
  }

  Future<void> _reloginWithDeviceId() async {
    final deviceId = await SecureStorage.getDeviceId();
    if (deviceId != null && deviceId.isNotEmpty) {
      await AuthApi.login(deviceId: deviceId);
    }
  }

  void _loadSupportedChains() async {
    try {
      final result = await ChainsApi.getSupportedChains();
      if (result.isSuccess && result.data != null) {
        ChainConfig.loadFromRemote(result.data!);
      }
    } catch (_) {}
  }

  Future<void> _refreshBalanceInBackground(String address) async {
    try {
      await Services.balance.refresh(address);
    } catch (_) {}
  }

  @override
  void dispose() {
    Services.push.dispose();
    Services.presignPool.dispose();
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
        navigatorKey: _navigatorKey,
        title: 'CoWallet',
        debugShowCheckedModeBanner: false,
        theme: cwTheme(),
        initialRoute: _initialRoute,
        onGenerateRoute: AppRouter.onGenerateRoute,
      ),
    );
  }
}
