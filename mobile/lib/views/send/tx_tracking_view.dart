import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import '../../api/tx_api.dart';
import '../../l10n/strings.dart';
import '../../services/locator.dart';
import '../../theme/colors.dart';

enum TxTrackingStatus { pending, confirmed, failed }

class TxTrackingView extends StatefulWidget {
  final String txHash;
  final String toAddress;
  final String amount;
  final String token;

  const TxTrackingView({
    super.key,
    required this.txHash,
    required this.toAddress,
    required this.amount,
    required this.token,
  });

  @override
  State<TxTrackingView> createState() => _TxTrackingViewState();
}

class _TxTrackingViewState extends State<TxTrackingView> {
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
      final result = await TxApi.getStatus(widget.txHash);
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
            color: CwColors.success,
            shape: BoxShape.circle,
          ),
          child: const Icon(Icons.check, color: Colors.white, size: 36),
        );
      case TxTrackingStatus.failed:
        return Container(
          width: 64,
          height: 64,
          decoration: const BoxDecoration(
            color: CwColors.danger,
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
        color: CwColors.bgSubtle,
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
    final valueWidget = Text(
      value,
      style: Theme.of(context).textTheme.labelLarge,
    );
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
                    const Icon(Icons.copy, size: 14, color: CwColors.ink4),
                  ],
                ),
              )
            : valueWidget,
      ],
    );
  }
}
