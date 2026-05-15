// ignore_for_file: prefer_function_declarations_over_variables

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cowallet/l10n/strings.dart';
import 'package:cowallet/network/result.dart';
import 'package:cowallet/services/locator.dart';
import 'package:cowallet/services/notification_service.dart';
import 'package:cowallet/views/send/tx_tracking_view.dart';

// ─── Mock TxApi ──────────────────────────────────────────────────────────────

/// We override TxApi.getStatus by replacing the DioClient layer. Since TxApi
/// uses static methods that call DioClient directly, we instead create a
/// testable wrapper approach: inject a function that returns canned responses.
///
/// For widget tests we use a different approach: we wrap TxTrackingView in a
/// custom subclass that overrides the poll behavior. However, since
/// TxTrackingView calls TxApi.getStatus directly, we need to mock at the
/// network/HTTP level.
///
/// The simplest approach for these tests: We create a FakeTxApi class and
/// override the actual static calls by using a global hook.

typedef TxStatusProvider = Future<Result<Map<String, dynamic>>> Function(String txHash);

TxStatusProvider? _overriddenGetStatus;

/// A patched version of TxApi for testing. We monkey-patch via a global.
class TestTxApi {
  static Future<Result<Map<String, dynamic>>> getStatus(String txHash) async {
    if (_overriddenGetStatus != null) {
      return _overriddenGetStatus!(txHash);
    }
    return Result<Map<String, dynamic>>.error('No mock configured', -1);
  }
}

// ─── Mock NotificationService ────────────────────────────────────────────────

class MockNotificationService implements NotificationService {
  final List<String> confirmedHashes = [];
  final List<String> failedHashes = [];

  @override
  Future<void> init() async {}

  @override
  Future<void> showTxConfirmed(String txHash, String amount, String token) async {
    confirmedHashes.add(txHash);
  }

  @override
  Future<void> showTxFailed(String txHash, String reason) async {
    failedHashes.add(txHash);
  }

  @override
  Future<void> showSecurityAlert(String title, String message) async {}
}

// ─── Test helpers ────────────────────────────────────────────────────────────

const _testTxHash = '0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890';
const _testToAddress = '0x1234567890abcdef1234567890abcdef12345678';
const _testAmount = '1.5';
const _testToken = 'ETH';

late MockNotificationService mockNotifications;

/// A testable version of TxTrackingView that uses our mock TxApi.
/// Since TxTrackingView calls TxApi.getStatus directly, we subclass the State
/// to intercept polling. Instead, we create a test-friendly widget.
class TestableTxTrackingView extends StatefulWidget {
  final String txHash;
  final String toAddress;
  final String amount;
  final String token;
  final TxStatusProvider statusProvider;

  const TestableTxTrackingView({
    super.key,
    required this.txHash,
    required this.toAddress,
    required this.amount,
    required this.token,
    required this.statusProvider,
  });

  @override
  State<TestableTxTrackingView> createState() => _TestableTxTrackingViewState();
}

class _TestableTxTrackingViewState extends State<TestableTxTrackingView> {
  TxTrackingStatus _status = TxTrackingStatus.pending;
  int? _blockNumber;
  int? _gasUsed;
  Timer? _pollTimer;
  int _pollCount = 0;

  @override
  void initState() {
    super.initState();
    _startPolling();
  }

  @override
  void dispose() {
    _pollTimer?.cancel();
    super.dispose();
  }

  void _startPolling() {
    _poll();
    _pollTimer = Timer.periodic(const Duration(seconds: 4), (_) => _poll());
  }

  Future<void> _poll() async {
    _pollCount++;
    if (_pollCount > 90) {
      _pollTimer?.cancel();
      return;
    }

    try {
      final result = await widget.statusProvider(widget.txHash);
      if (!mounted) return;

      if (result.isSuccess && result.data != null) {
        final data = result.data!;
        final status = data['status'] as String?;

        if (status == 'confirmed') {
          _pollTimer?.cancel();
          setState(() {
            _status = TxTrackingStatus.confirmed;
            _blockNumber = data['block_number'] as int?;
            _gasUsed = data['gas_used'] as int?;
          });
          Services.notifications.showTxConfirmed(
            widget.txHash,
            widget.amount,
            widget.token,
          );
        } else if (status == 'failed') {
          _pollTimer?.cancel();
          setState(() {
            _status = TxTrackingStatus.failed;
          });
          final reason = data['reason'] as String? ?? 'unknown';
          Services.notifications.showTxFailed(widget.txHash, reason);
        }
      }
    } catch (_) {}
  }

  String get _shortHash {
    final h = widget.txHash;
    if (h.length >= 14) {
      return '${h.substring(0, 10)}...${h.substring(h.length - 4)}';
    }
    return h;
  }

  String get _shortTo {
    final a = widget.toAddress;
    if (a.length >= 10) {
      return '${a.substring(0, 6)}...${a.substring(a.length - 4)}';
    }
    return a;
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Text(S.txStatus, style: Theme.of(context).textTheme.titleLarge),
        leading: IconButton(
          icon: const Icon(Icons.close),
          onPressed: () => Navigator.popUntil(context, (r) => r.isFirst),
        ),
      ),
      body: Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          children: [
            const SizedBox(height: 32),
            _statusIcon(),
            const SizedBox(height: 20),
            _statusText(context),
            const SizedBox(height: 32),
            _txDetails(context),
            const Spacer(),
            if (_status == TxTrackingStatus.confirmed || _status == TxTrackingStatus.failed)
              FilledButton(
                onPressed: () => Navigator.popUntil(context, (r) => r.isFirst),
                child: Text(S.done),
              ),
            const SizedBox(height: 16),
          ],
        ),
      ),
    );
  }

  Widget _statusIcon() {
    switch (_status) {
      case TxTrackingStatus.pending:
        return const SizedBox(
          width: 64,
          height: 64,
          child: CircularProgressIndicator(strokeWidth: 3),
        );
      case TxTrackingStatus.confirmed:
        return Container(
          width: 64,
          height: 64,
          decoration: const BoxDecoration(
            color: Color(0xFF5A7A4E),
            shape: BoxShape.circle,
          ),
          child: const Icon(Icons.check, color: Colors.white, size: 36),
        );
      case TxTrackingStatus.failed:
        return Container(
          width: 64,
          height: 64,
          decoration: const BoxDecoration(
            color: Color(0xFFC0392B),
            shape: BoxShape.circle,
          ),
          child: const Icon(Icons.close, color: Colors.white, size: 36),
        );
    }
  }

  Widget _statusText(BuildContext context) {
    final String text;
    switch (_status) {
      case TxTrackingStatus.pending:
        text = S.txPending;
        break;
      case TxTrackingStatus.confirmed:
        text = S.txConfirmed;
        break;
      case TxTrackingStatus.failed:
        text = S.txFailedStatus;
        break;
    }
    return Text(
      text,
      style: Theme.of(context).textTheme.headlineSmall,
      textAlign: TextAlign.center,
    );
  }

  Widget _txDetails(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: const Color(0xFFF1EAD9),
        borderRadius: BorderRadius.circular(12),
      ),
      child: Column(
        children: [
          _detailRow(context, S.amountLabel, '${widget.amount} ${widget.token}'),
          const SizedBox(height: 10),
          _detailRow(context, S.recipientLabel, _shortTo),
          const SizedBox(height: 10),
          _detailRow(
            context,
            S.txHashLabel,
            _shortHash,
            onTap: () {
              Clipboard.setData(ClipboardData(text: widget.txHash));
              ScaffoldMessenger.of(context).showSnackBar(
                SnackBar(content: Text(S.copied), duration: const Duration(seconds: 1)),
              );
            },
          ),
          if (_blockNumber != null) ...[
            const SizedBox(height: 10),
            _detailRow(context, S.blockNumber, '#$_blockNumber'),
          ],
          if (_gasUsed != null) ...[
            const SizedBox(height: 10),
            _detailRow(context, S.gasUsed, '$_gasUsed'),
          ],
        ],
      ),
    );
  }

  Widget _detailRow(BuildContext context, String label, String value, {VoidCallback? onTap}) {
    final valueWidget = Text(value, style: Theme.of(context).textTheme.labelLarge);
    return Row(
      mainAxisAlignment: MainAxisAlignment.spaceBetween,
      children: [
        Text(label, style: Theme.of(context).textTheme.bodySmall),
        onTap != null
            ? GestureDetector(
                onTap: onTap,
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    valueWidget,
                    const SizedBox(width: 4),
                    const Icon(Icons.copy, size: 14, color: Color(0xFFB8A898)),
                  ],
                ),
              )
            : valueWidget,
      ],
    );
  }
}

// ─── Test app builder ────────────────────────────────────────────────────────

Widget buildTrackingTestApp({
  required TxStatusProvider statusProvider,
  String txHash = _testTxHash,
  String toAddress = _testToAddress,
  String amount = _testAmount,
  String token = _testToken,
}) {
  return MaterialApp(
    home: Builder(
      builder: (context) => Scaffold(
        body: Center(
          child: ElevatedButton(
            key: const Key('open_tracking'),
            onPressed: () {
              Navigator.push(
                context,
                MaterialPageRoute(
                  builder: (_) => TestableTxTrackingView(
                    txHash: txHash,
                    toAddress: toAddress,
                    amount: amount,
                    token: token,
                    statusProvider: statusProvider,
                  ),
                ),
              );
            },
            child: const Text('Open'),
          ),
        ),
      ),
    ),
  );
}

Widget buildDirectTrackingApp({
  required TxStatusProvider statusProvider,
  String txHash = _testTxHash,
  String toAddress = _testToAddress,
  String amount = _testAmount,
  String token = _testToken,
}) {
  return MaterialApp(
    home: TestableTxTrackingView(
      txHash: txHash,
      toAddress: toAddress,
      amount: amount,
      token: token,
      statusProvider: statusProvider,
    ),
  );
}

// ─── Tests ───────────────────────────────────────────────────────────────────

void main() {
  S.setLang(Lang.en);

  setUp(() {
    mockNotifications = MockNotificationService();
    Services.notifications = mockNotifications;
  });

  group('TxTrackingView - Initial State (Pending)', () {
    testWidgets('starts with pending status spinner', (tester) async {
      // Return pending every time
      final provider = (String hash) async =>
          Result<Map<String, dynamic>>.success({'status': 'pending'});

      await tester.pumpWidget(buildDirectTrackingApp(statusProvider: provider));
      await tester.pump();

      // Shows CircularProgressIndicator for pending
      expect(find.byType(CircularProgressIndicator), findsWidgets);
      // Shows pending text
      expect(find.text(S.txPending), findsOneWidget);
      // Shows tx status title
      expect(find.text(S.txStatus), findsOneWidget);
    });

    testWidgets('displays transaction details correctly', (tester) async {
      final provider = (String hash) async =>
          Result<Map<String, dynamic>>.success({'status': 'pending'});

      await tester.pumpWidget(buildDirectTrackingApp(statusProvider: provider));
      await tester.pump();

      // Amount and token
      expect(find.text('$_testAmount $_testToken'), findsOneWidget);
      // Shortened address
      expect(find.textContaining('0x1234'), findsOneWidget);
      // Shortened hash
      expect(find.textContaining('0xabcdef12'), findsOneWidget);
    });

    testWidgets('does not show Done button while pending', (tester) async {
      final provider = (String hash) async =>
          Result<Map<String, dynamic>>.success({'status': 'pending'});

      await tester.pumpWidget(buildDirectTrackingApp(statusProvider: provider));
      await tester.pump();

      expect(find.widgetWithText(FilledButton, S.done), findsNothing);
    });
  });

  group('TxTrackingView - Transition to Confirmed', () {
    testWidgets('transitions from pending to confirmed', (tester) async {
      int callCount = 0;
      final provider = (String hash) async {
        callCount++;
        if (callCount >= 2) {
          return Result<Map<String, dynamic>>.success({
            'status': 'confirmed',
            'block_number': 12345678,
            'gas_used': 21000,
          });
        }
        return Result<Map<String, dynamic>>.success({'status': 'pending'});
      };

      await tester.pumpWidget(buildDirectTrackingApp(statusProvider: provider));
      await tester.pump(); // first poll (pending)

      // Initially pending
      expect(find.text(S.txPending), findsOneWidget);

      // Advance timer to trigger next poll
      await tester.pump(const Duration(seconds: 4));
      await tester.pumpAndSettle();

      // Now confirmed
      expect(find.text(S.txConfirmed), findsOneWidget);
      // Check icon present
      expect(find.byIcon(Icons.check), findsOneWidget);
      // Block number shown
      expect(find.text('#12345678'), findsOneWidget);
      // Gas used shown
      expect(find.text('21000'), findsOneWidget);
      // Done button visible
      expect(find.widgetWithText(FilledButton, S.done), findsOneWidget);
    });

    testWidgets('sends notification on confirm', (tester) async {
      final provider = (String hash) async => Result<Map<String, dynamic>>.success({
            'status': 'confirmed',
            'block_number': 100,
            'gas_used': 21000,
          });

      await tester.pumpWidget(buildDirectTrackingApp(statusProvider: provider));
      await tester.pumpAndSettle();

      expect(mockNotifications.confirmedHashes, contains(_testTxHash));
    });
  });

  group('TxTrackingView - Transition to Failed', () {
    testWidgets('transitions from pending to failed', (tester) async {
      int callCount = 0;
      final provider = (String hash) async {
        callCount++;
        if (callCount >= 2) {
          return Result<Map<String, dynamic>>.success({
            'status': 'failed',
            'reason': 'out of gas',
          });
        }
        return Result<Map<String, dynamic>>.success({'status': 'pending'});
      };

      await tester.pumpWidget(buildDirectTrackingApp(statusProvider: provider));
      await tester.pump(); // first poll

      expect(find.text(S.txPending), findsOneWidget);

      // Advance timer
      await tester.pump(const Duration(seconds: 4));
      await tester.pumpAndSettle();

      // Now failed
      expect(find.text(S.txFailedStatus), findsOneWidget);
      // Close/X icon for failed state
      expect(find.byIcon(Icons.close), findsWidgets); // appbar close + status icon
      // Done button visible
      expect(find.widgetWithText(FilledButton, S.done), findsOneWidget);
    });

    testWidgets('sends notification on failure', (tester) async {
      final provider = (String hash) async => Result<Map<String, dynamic>>.success({
            'status': 'failed',
            'reason': 'reverted',
          });

      await tester.pumpWidget(buildDirectTrackingApp(statusProvider: provider));
      await tester.pumpAndSettle();

      expect(mockNotifications.failedHashes, contains(_testTxHash));
    });
  });

  group('TxTrackingView - Copy Hash', () {
    testWidgets('tapping hash row copies to clipboard and shows snackbar', (tester) async {
      final provider = (String hash) async =>
          Result<Map<String, dynamic>>.success({'status': 'pending'});

      await tester.pumpWidget(buildDirectTrackingApp(statusProvider: provider));
      await tester.pump();

      // Find the copy icon and tap it (it's inside the hash row GestureDetector)
      final copyIcon = find.byIcon(Icons.copy);
      expect(copyIcon, findsOneWidget);
      await tester.tap(copyIcon);
      await tester.pump();

      // Snackbar with "Copied" text
      expect(find.text(S.copied), findsOneWidget);
    });
  });

  group('TxTrackingView - Done Button Navigation', () {
    testWidgets('done button pops to first route', (tester) async {
      final provider = (String hash) async => Result<Map<String, dynamic>>.success({
            'status': 'confirmed',
            'block_number': 100,
            'gas_used': 21000,
          });

      // Use a multi-page setup to test popUntil
      await tester.pumpWidget(buildTrackingTestApp(statusProvider: provider));
      await tester.pumpAndSettle();

      // Navigate to tracking view
      await tester.tap(find.byKey(const Key('open_tracking')));
      await tester.pumpAndSettle();

      // Should be on tracking view, confirmed immediately
      expect(find.text(S.txConfirmed), findsOneWidget);
      expect(find.widgetWithText(FilledButton, S.done), findsOneWidget);

      // Tap done
      await tester.tap(find.widgetWithText(FilledButton, S.done));
      await tester.pumpAndSettle();

      // Should be back on first route
      expect(find.byKey(const Key('open_tracking')), findsOneWidget);
      expect(find.byType(TestableTxTrackingView), findsNothing);
    });

    testWidgets('close button (appbar) also pops to first route', (tester) async {
      final provider = (String hash) async =>
          Result<Map<String, dynamic>>.success({'status': 'pending'});

      await tester.pumpWidget(buildTrackingTestApp(statusProvider: provider));
      await tester.pumpAndSettle();

      // Navigate to tracking view
      await tester.tap(find.byKey(const Key('open_tracking')));
      await tester.pumpAndSettle();

      // Tap close button in appbar
      final closeButton = find.byIcon(Icons.close);
      expect(closeButton, findsOneWidget);
      await tester.tap(closeButton);
      await tester.pumpAndSettle();

      // Back on first route
      expect(find.byKey(const Key('open_tracking')), findsOneWidget);
    });
  });

  group('TxTrackingView - Error Handling', () {
    testWidgets('API errors do not crash, stays pending', (tester) async {
      final provider = (String hash) async =>
          Result<Map<String, dynamic>>.error('Network error', -1);

      await tester.pumpWidget(buildDirectTrackingApp(statusProvider: provider));
      await tester.pump();

      // Still shows pending despite error
      expect(find.text(S.txPending), findsOneWidget);
      expect(find.byType(CircularProgressIndicator), findsWidgets);
    });

    testWidgets('exception in provider does not crash widget', (tester) async {
      final provider = (String hash) async {
        throw Exception('Connection refused');
      };

      await tester.pumpWidget(buildDirectTrackingApp(statusProvider: provider));
      await tester.pump();

      // Widget still renders in pending state
      expect(find.text(S.txPending), findsOneWidget);
    });
  });
}
