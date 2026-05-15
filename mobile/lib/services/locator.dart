import 'package:flutter/widgets.dart';
import '../bridge/frb_generated/frb_generated.dart';
import '../widgets/pin_verify_dialog.dart';
import '../platform/biometrics.dart';
import '../platform/biometrics_impl.dart';
import '../platform/cloud_backup.dart';
import '../platform/secure_storage.dart';
import '../platform/secure_storage_impl.dart';
import '../api/mpc_api.dart';
import 'backup_shard_service.dart';
import 'contacts_service.dart';
import 'settings_service.dart';
import 'wallet_service.dart';
import 'chain_service.dart';
import 'balance_service.dart';
import 'tx_service.dart';
import 'intent_executor.dart';
import 'gas_service.dart';
import 'notification_service.dart';
import 'push_service.dart';
import 'tx_history_service.dart';
import 'mpc_wallet_service.dart';
import 'policy_service.dart';
import 'presign_pool_service.dart';

class Services {
  static final navigatorKey = GlobalKey<NavigatorState>();
  static late BiometricService biometrics;
  static late SecureStorageService storage;
  static late WalletService wallet;
  static late MpcWalletService mpcWallet;
  static late ChainService chain;
  static late BalanceService balance;
  static late TxService tx;
  static late IntentExecutor intent;
  static late GasService gas;
  static late TxHistoryService txHistory;
  static late BackupShardService backup;
  static late ContactsService contacts;
  static late NotificationService notifications;
  static late PushService push;
  static late SettingsService settings;
  static late PolicyService policy;
  static late PresignPoolService presignPool;

  // API clients (stateless, no initialization needed)
  static final mpcApi = MpcApi();

  static Future<void> init() async {
    await RustLib.init();
    storage = FlutterSecureStorageService();
    biometrics = LocalAuthBiometricService();
    backup = BackupShardService(PlatformCloudBackup());
    mpcWallet = MpcWalletService();
    wallet = mpcWallet;
    chain = JsonRpcChainService();
    balance = BalanceService();
    gas = GasService(chain);
    tx = MpcTxService(
      wallet: wallet,
      chain: chain,
    );
    txHistory = TxHistoryService(storage: storage, chain: chain);
    await txHistory.load();
    contacts = ContactsService();
    await contacts.load();
    notifications = NotificationService();
    await notifications.init();
    push = PushService();
    await push.init();
    settings = SettingsService();
    await settings.init();
    intent = IntentExecutor(
      wallet: wallet,
      balance: balance,
      tx: tx,
      gas: gas,
      txHistory: txHistory,
      chain: chain,
    );
    policy = PolicyService();
    presignPool = PresignPoolService();
  }

  /// Unified authentication: biometric if user enabled it, otherwise app PIN.
  /// All sensitive operations MUST use this — never call biometrics.authenticate directly.
  static Future<bool> authenticate({required String reason}) async {
    final biometricEnabled = await biometrics.isEnabled();
    if (biometricEnabled) {
      return biometrics.authenticate(reason: reason);
    }
    final ctx = navigatorKey.currentContext;
    if (ctx == null) return false;
    return PinVerifyDialog.show(ctx, reason: reason);
  }
}
