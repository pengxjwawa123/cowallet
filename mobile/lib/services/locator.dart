import '../bridge/frb_generated/frb_generated.dart';
import '../platform/biometrics.dart';
import '../platform/biometrics_impl.dart';
import '../platform/cloud_backup.dart';
import '../platform/secure_storage.dart';
import '../platform/secure_storage_impl.dart';
import '../api/mpc_api.dart';
import 'backup_shard_service.dart';
import 'wallet_service.dart';
import 'chain_service.dart';
import 'balance_service.dart';
import 'tx_service.dart';
import 'intent_executor.dart';
import 'gas_service.dart';
import 'tx_history_service.dart';
import 'mpc_wallet_service.dart';

class Services {
  static late final BiometricService biometrics;
  static late final SecureStorageService storage;
  static late final WalletService wallet;
  static late final MpcWalletService mpcWallet;
  static late final ChainService chain;
  static late final BalanceService balance;
  static late final TxService tx;
  static late final IntentExecutor intent;
  static late final GasService gas;
  static late final TxHistoryService txHistory;
  static late final BackupShardService backup;

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
    balance = BalanceService(chain);
    gas = GasService(chain);
    tx = MpcTxService(
      wallet: wallet,
      chain: chain,
      biometrics: biometrics,
    );
    txHistory = TxHistoryService(storage: storage, chain: chain);
    await txHistory.load();
    intent = IntentExecutor(
      wallet: wallet,
      balance: balance,
      tx: tx,
      gas: gas,
      txHistory: txHistory,
    );
  }
}
