import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cowallet/l10n/strings.dart';
import 'package:cowallet/models/tx_record.dart';
import 'package:cowallet/services/gas_service.dart';
import 'package:cowallet/services/locator.dart';
import 'package:cowallet/services/balance_service.dart';
import 'package:cowallet/services/chain_service.dart';
import 'package:cowallet/services/notification_service.dart';
import 'package:cowallet/services/tx_history_service.dart';
import 'package:cowallet/services/tx_service.dart';
import 'package:cowallet/services/wallet_service.dart';
import 'package:cowallet/platform/biometrics.dart';
import 'package:cowallet/views/send/send_view.dart';
import 'package:cowallet/views/send/tx_tracking_view.dart';

// ─── Mocks ───────────────────────────────────────────────────────────────────

class MockBiometricService implements BiometricService {
  bool shouldAuthenticate = true;

  @override
  Future<bool> isAvailable() async => true;

  @override
  Future<bool> authenticate({required String reason}) async => shouldAuthenticate;

  @override
  Future<List<String>> getAvailableBiometrics() async => ['face'];

  @override
  Future<bool> isEnabled() async => true;

  @override
  Future<void> setEnabled(bool enabled) async {}

  @override
  Future<bool> hasEnrolledBiometrics() async => true;

  @override
  Future<String> getPrimaryBiometricType() async => 'Face ID';
}

class MockWalletService implements WalletService {
  @override
  Future<String> getAddress() async => '0x1234567890abcdef1234567890abcdef12345678';

  @override
  Future<bool> hasWallet() async => true;

  @override
  Future<void> deleteWallet() async {}

  @override
  Future<List<int>> sign(List<int> msgHash) async => List.filled(65, 0);

  @override
  Future<SignResult> signWithSession(List<int> msgHash) async =>
      SignResult(signature: List.filled(65, 0), sessionId: 'session-123');
}

class MockGasService implements GasService {
  bool shouldFail = false;

  @override
  void clearCache() {}

  @override
  Future<GasEstimate> estimate({
    required String from,
    required String to,
    required BigInt value,
    String? data,
  }) async {
    if (shouldFail) throw Exception('Gas estimation failed');
    return GasEstimate(
      gasLimit: BigInt.from(21000),
      maxFeePerGas: BigInt.from(2000000000),
      maxPriorityFeePerGas: BigInt.from(1500000000),
      totalWei: BigInt.from(42000000000000),
      formattedEth: '0.000042 ETH',
      formattedUsd: '\$0.13',
    );
  }
}

class MockTxService implements TxService {
  bool shouldFail = false;
  String lastTxHash = '0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890';

  @override
  Future<String> signAndSend({
    required String to,
    required BigInt value,
    BigInt? gasLimit,
    BigInt? maxFeePerGas,
    BigInt? maxPriorityFeePerGas,
    String? data,
    int? chainId,
  }) async {
    if (shouldFail) throw Exception('TX failed');
    return lastTxHash;
  }

  @override
  Future<String> sendErc20({
    required String to,
    required String tokenContract,
    required BigInt amount,
    BigInt? gasLimit,
    BigInt? maxFeePerGas,
    BigInt? maxPriorityFeePerGas,
    int? chainId,
  }) async {
    if (shouldFail) throw Exception('ERC20 TX failed');
    return lastTxHash;
  }
}

class MockChainService implements ChainService {
  @override
  String tokenContract(String symbol) => '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913';

  @override
  ChainConfig get currentConfig => ChainConfig.base;

  @override
  Future<BigInt> getEthBalance(String address) async => BigInt.from(1000000000000000000);

  @override
  Future<BigInt> getTokenBalance(String address, String tokenContract) async => BigInt.from(1000000);

  @override
  Future<String> sendRawTransaction(String signedTxHex) async => '0xhash';

  @override
  Future<int> getTransactionCount(String address) async => 0;

  @override
  Future<BigInt> getGasPrice() async => BigInt.from(1000000000);

  @override
  Future<BigInt> estimateGas(Map<String, dynamic> txParams) async => BigInt.from(21000);

  @override
  Future<BigInt?> getBaseFee() async => BigInt.from(1000000000);

  @override
  Future<BigInt> getMaxPriorityFeePerGas() async => BigInt.from(1500000000);

  @override
  Future<Map<String, dynamic>?> getTransactionReceipt(String txHash) async => null;
}

class MockTxHistoryService extends ChangeNotifier implements TxHistoryService {
  @override
  List<TxRecord> get records => [];

  @override
  Future<void> add(TxRecord record) async {}

  @override
  Future<void> load() async {}

  @override
  Future<void> refreshStatuses() async {}
}

class MockNotificationService implements NotificationService {
  @override
  Future<void> init() async {}

  @override
  Future<void> showTxConfirmed(String txHash, String amount, String token) async {}

  @override
  Future<void> showTxFailed(String txHash, String reason) async {}

  @override
  Future<void> showSecurityAlert(String title, String message) async {}
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

late MockBiometricService mockBiometrics;
late MockWalletService mockWallet;
late MockGasService mockGas;
late MockTxService mockTx;
late MockChainService mockChain;
late MockTxHistoryService mockTxHistory;
late MockNotificationService mockNotifications;
late BalanceService mockBalance;

void setupMocks() {
  mockBiometrics = MockBiometricService();
  mockWallet = MockWalletService();
  mockGas = MockGasService();
  mockTx = MockTxService();
  mockChain = MockChainService();
  mockTxHistory = MockTxHistoryService();
  mockNotifications = MockNotificationService();
  mockBalance = BalanceService();

  Services.biometrics = mockBiometrics;
  Services.wallet = mockWallet;
  Services.gas = mockGas;
  Services.tx = mockTx;
  Services.chain = mockChain;
  Services.txHistory = mockTxHistory;
  Services.notifications = mockNotifications;
  Services.balance = mockBalance;
}

Widget buildTestApp({Widget? child}) {
  return MaterialApp(
    home: child ?? const SendView(),
  );
}

// ─── Tests ───────────────────────────────────────────────────────────────────

void main() {
  S.setLang(Lang.en);

  setUp(setupMocks);

  group('SendView - Validation', () {
    testWidgets('shows toast when address is empty and send pressed', (tester) async {
      await tester.pumpWidget(buildTestApp());
      await tester.pumpAndSettle();

      // Tap the confirm/send button without filling fields
      final sendButton = find.widgetWithText(FilledButton, S.confirmTransfer);
      expect(sendButton, findsOneWidget);
      await tester.tap(sendButton);
      await tester.pump();

      // Should show "enter address" snackbar toast
      expect(find.text(S.enterAddress), findsOneWidget);
    });

    testWidgets('shows toast when amount is empty', (tester) async {
      await tester.pumpWidget(buildTestApp());
      await tester.pumpAndSettle();

      // Fill only address
      final addressField = find.byType(TextField).first;
      await tester.enterText(addressField, '0x1234567890abcdef1234567890abcdef12345678');
      await tester.pump();

      // Tap send
      final sendButton = find.widgetWithText(FilledButton, S.confirmTransfer);
      await tester.tap(sendButton);
      await tester.pump();

      // Should show "enter valid amount" snackbar toast
      expect(find.text(S.enterValidAmount), findsOneWidget);
    });

    testWidgets('shows toast when amount is zero', (tester) async {
      await tester.pumpWidget(buildTestApp());
      await tester.pumpAndSettle();

      // Fill address and zero amount
      final fields = find.byType(TextField);
      await tester.enterText(fields.first, '0x1234567890abcdef1234567890abcdef12345678');
      await tester.enterText(fields.at(1), '0');
      await tester.pump();

      final sendButton = find.widgetWithText(FilledButton, S.confirmTransfer);
      await tester.tap(sendButton);
      await tester.pump();

      expect(find.text(S.enterValidAmount), findsOneWidget);
    });
  });

  group('SendView - Confirmation Dialog', () {
    testWidgets('shows confirmation dialog with valid input', (tester) async {
      await tester.pumpWidget(buildTestApp());
      await tester.pumpAndSettle();

      // Fill valid address and amount
      final fields = find.byType(TextField);
      await tester.enterText(fields.first, '0x1234567890abcdef1234567890abcdef12345678');
      await tester.enterText(fields.at(1), '0.5');
      await tester.pump();

      // Tap send
      final sendButton = find.widgetWithText(FilledButton, S.confirmTransfer);
      await tester.tap(sendButton);
      await tester.pumpAndSettle();

      // Confirm dialog appears with correct content
      expect(find.byType(AlertDialog), findsOneWidget);
      expect(find.text(S.confirmTransfer), findsWidgets); // title + button
      expect(find.textContaining('0.5'), findsOneWidget);
      expect(find.textContaining('ETH'), findsOneWidget);
    });

    testWidgets('dismiss dialog on cancel returns without sending', (tester) async {
      await tester.pumpWidget(buildTestApp());
      await tester.pumpAndSettle();

      // Fill valid data
      final fields = find.byType(TextField);
      await tester.enterText(fields.first, '0x1234567890abcdef1234567890abcdef12345678');
      await tester.enterText(fields.at(1), '1.0');
      await tester.pump();

      // Tap send to open dialog
      final sendButton = find.widgetWithText(FilledButton, S.confirmTransfer);
      await tester.tap(sendButton);
      await tester.pumpAndSettle();

      // Tap cancel
      final cancelButton = find.widgetWithText(TextButton, S.cancel);
      expect(cancelButton, findsOneWidget);
      await tester.tap(cancelButton);
      await tester.pumpAndSettle();

      // Dialog dismissed, still on send view
      expect(find.byType(AlertDialog), findsNothing);
      expect(find.byType(SendView), findsOneWidget);
    });
  });

  group('SendView - Biometric Gate', () {
    testWidgets('biometric failure aborts send and shows toast', (tester) async {
      mockBiometrics.shouldAuthenticate = false;

      await tester.pumpWidget(buildTestApp());
      await tester.pumpAndSettle();

      // Fill valid data
      final fields = find.byType(TextField);
      await tester.enterText(fields.first, '0x1234567890abcdef1234567890abcdef12345678');
      await tester.enterText(fields.at(1), '1.0');
      await tester.pump();

      // Tap send to open dialog
      final sendButton = find.widgetWithText(FilledButton, S.confirmTransfer);
      await tester.tap(sendButton);
      await tester.pumpAndSettle();

      // Confirm in dialog
      final confirmButtons = find.widgetWithText(FilledButton, S.confirmTransfer);
      // The dialog has a FilledButton with same text
      await tester.tap(confirmButtons.last);
      await tester.pumpAndSettle();

      // Biometric fails -> shows toast, stays on SendView
      expect(find.text(S.bioAuthFailed), findsOneWidget);
      expect(find.byType(SendView), findsOneWidget);
    });

    testWidgets('biometric success proceeds to transaction', (tester) async {
      mockBiometrics.shouldAuthenticate = true;

      await tester.pumpWidget(buildTestApp());
      await tester.pumpAndSettle();

      // Fill valid data
      final fields = find.byType(TextField);
      await tester.enterText(fields.first, '0x1234567890abcdef1234567890abcdef12345678');
      await tester.enterText(fields.at(1), '1.0');
      await tester.pump();

      // Tap send to open dialog
      final sendButton = find.widgetWithText(FilledButton, S.confirmTransfer);
      await tester.tap(sendButton);
      await tester.pumpAndSettle();

      // Confirm in dialog
      final confirmButtons = find.widgetWithText(FilledButton, S.confirmTransfer);
      await tester.tap(confirmButtons.last);
      await tester.pumpAndSettle();

      // Biometric succeeds, tx succeeds -> navigates to TxTrackingView
      expect(find.byType(TxTrackingView), findsOneWidget);
    });
  });

  group('SendView - Navigation to Tracking', () {
    testWidgets('successful send navigates to TxTrackingView with correct params', (tester) async {
      mockBiometrics.shouldAuthenticate = true;

      await tester.pumpWidget(buildTestApp());
      await tester.pumpAndSettle();

      // Fill valid data
      final fields = find.byType(TextField);
      await tester.enterText(fields.first, '0x1234567890abcdef1234567890abcdef12345678');
      await tester.enterText(fields.at(1), '2.5');
      await tester.pump();

      // Tap send to open dialog
      final sendButton = find.widgetWithText(FilledButton, S.confirmTransfer);
      await tester.tap(sendButton);
      await tester.pumpAndSettle();

      // Confirm in dialog
      final confirmButtons = find.widgetWithText(FilledButton, S.confirmTransfer);
      await tester.tap(confirmButtons.last);
      await tester.pumpAndSettle();

      // Now on TxTrackingView
      expect(find.byType(TxTrackingView), findsOneWidget);
      // Shows the amount
      expect(find.textContaining('2.5'), findsOneWidget);
      // Shows token
      expect(find.textContaining('ETH'), findsWidgets);
    });

    testWidgets('failed transaction shows error toast and stays on SendView', (tester) async {
      mockBiometrics.shouldAuthenticate = true;
      mockTx.shouldFail = true;

      await tester.pumpWidget(buildTestApp());
      await tester.pumpAndSettle();

      // Fill valid data
      final fields = find.byType(TextField);
      await tester.enterText(fields.first, '0x1234567890abcdef1234567890abcdef12345678');
      await tester.enterText(fields.at(1), '1.0');
      await tester.pump();

      // Tap send to open dialog
      final sendButton = find.widgetWithText(FilledButton, S.confirmTransfer);
      await tester.tap(sendButton);
      await tester.pumpAndSettle();

      // Confirm in dialog
      final confirmButtons = find.widgetWithText(FilledButton, S.confirmTransfer);
      await tester.tap(confirmButtons.last);
      await tester.pumpAndSettle();

      // TX failed -> error toast shown, still on SendView
      expect(find.textContaining(S.txFailed), findsOneWidget);
      expect(find.byType(SendView), findsOneWidget);
    });
  });

  group('SendView - UI elements', () {
    testWidgets('renders all essential UI elements', (tester) async {
      await tester.pumpWidget(buildTestApp());
      await tester.pumpAndSettle();

      // App bar title
      expect(find.text(S.sendTitle), findsOneWidget);
      // Recipient label
      expect(find.text(S.recipient), findsOneWidget);
      // Amount label
      expect(find.text(S.amount), findsOneWidget);
      // Network info
      expect(find.text(S.network), findsOneWidget);
      expect(find.text('Base'), findsOneWidget);
      // Signing method
      expect(find.text('MPC 2-of-3'), findsOneWidget);
      // Send button
      expect(find.widgetWithText(FilledButton, S.confirmTransfer), findsOneWidget);
    });

    testWidgets('token dropdown shows ETH, USDC, USDT', (tester) async {
      await tester.pumpWidget(buildTestApp());
      await tester.pumpAndSettle();

      // Find the dropdown and tap it
      final dropdown = find.byType(DropdownButton<String>);
      expect(dropdown, findsOneWidget);

      await tester.tap(dropdown);
      await tester.pumpAndSettle();

      // All token options visible
      expect(find.text('ETH'), findsWidgets);
      expect(find.text('USDC'), findsOneWidget);
      expect(find.text('USDT'), findsOneWidget);
    });
  });
}
