import '../platform/biometrics.dart';
import '../platform/biometrics_impl.dart';
import '../platform/secure_storage.dart';
import '../platform/secure_storage_impl.dart';
import 'wallet_service.dart';
import 'chain_service.dart';
import 'balance_service.dart';
import 'tx_service.dart';
import 'intent_executor.dart';
import 'gas_service.dart';
import 'tx_history_service.dart';

class Services {
  static late final BiometricService biometrics;
  static late final SecureStorageService storage;
  static late final WalletService wallet;
  static late final ChainService chain;
  static late final BalanceService balance;
  static late final TxService tx;
  static late final IntentExecutor intent;
  static late final GasService gas;
  static late final TxHistoryService txHistory;

  static Future<void> init() async {
    storage = FlutterSecureStorageService();
    biometrics = LocalAuthBiometricService();
    wallet = DartWalletService(storage);
    chain = JsonRpcChainService();
    balance = BalanceService(chain);
    gas = GasService(chain);
    tx = DartTxService(
      wallet: wallet,
      chain: chain,
      biometrics: biometrics,
      storage: storage,
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
