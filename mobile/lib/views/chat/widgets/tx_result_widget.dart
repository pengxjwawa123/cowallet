import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import '../../../services/tx_tracker_service.dart';
import '../../../theme/colors.dart';
import '../../../widgets/top_toast.dart';

class ChatTxResultWidget extends StatefulWidget {
  final String txHash;
  final bool success;
  final String? amount;
  final String? token;
  final TxTrackerService? tracker;

  const ChatTxResultWidget({
    super.key,
    required this.txHash,
    this.success = true,
    this.amount,
    this.token,
    this.tracker,
  });

  @override
  State<ChatTxResultWidget> createState() => _ChatTxResultWidgetState();
}

class _ChatTxResultWidgetState extends State<ChatTxResultWidget> {
  TxStatus _status = TxStatus.broadcast;
  int? _confirmations;
  StreamSubscription<TxStatusInfo>? _subscription;

  @override
  void initState() {
    super.initState();
    if (widget.success && widget.tracker != null) {
      _subscription = widget.tracker!.track(widget.txHash).listen((info) {
        if (mounted) {
          setState(() {
            _status = info.status;
            _confirmations = info.confirmations;
          });
        }
      });
    } else if (!widget.success) {
      _status = TxStatus.failed;
    }
  }

  @override
  void dispose() {
    _subscription?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final shortHash = widget.txHash.length >= 16
        ? '${widget.txHash.substring(0, 10)}...${widget.txHash.substring(widget.txHash.length - 6)}'
        : widget.txHash;

    final isConfirmed = _status == TxStatus.confirmed;
    final isFailed = _status == TxStatus.failed;
    final isPending = _status == TxStatus.pending || _status == TxStatus.broadcast;

    final Color statusColor;
    final IconData statusIcon;
    final String statusText;

    if (isConfirmed) {
      statusColor = CwColors.success;
      statusIcon = Icons.check_circle;
      statusText = _confirmations != null ? '已确认 ($_confirmations blocks)' : '已确认';
    } else if (isFailed) {
      statusColor = CwColors.danger;
      statusIcon = Icons.error;
      statusText = '交易失败';
    } else {
      statusColor = CwColors.ink3;
      statusIcon = Icons.schedule;
      statusText = '确认中...';
    }

    return Container(
      margin: const EdgeInsets.only(bottom: 12),
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: statusColor.withValues(alpha: 0.06),
        borderRadius: BorderRadius.circular(16),
        border: Border.all(
          color: statusColor.withValues(alpha: 0.3),
        ),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              if (isPending)
                SizedBox(
                  width: 18,
                  height: 18,
                  child: CircularProgressIndicator(
                    strokeWidth: 2,
                    valueColor: AlwaysStoppedAnimation<Color>(statusColor),
                  ),
                )
              else
                Icon(statusIcon, size: 18, color: statusColor),
              const SizedBox(width: 8),
              Text(
                statusText,
                style: TextStyle(
                  fontSize: 14,
                  fontWeight: FontWeight.w600,
                  color: statusColor,
                ),
              ),
            ],
          ),
          if (widget.amount != null && widget.token != null) ...[
            const SizedBox(height: 8),
            Text(
              '${widget.amount} ${widget.token}',
              style: const TextStyle(
                fontSize: 18,
                fontWeight: FontWeight.w600,
                color: CwColors.ink1,
              ),
            ),
          ],
          const SizedBox(height: 8),
          GestureDetector(
            onTap: () {
              Clipboard.setData(ClipboardData(text: widget.txHash));
              showTopToast(context, '交易哈希已复制');
            },
            child: Row(
              children: [
                Text(
                  'Tx: $shortHash',
                  style: const TextStyle(
                    fontSize: 12,
                    fontFamily: 'JetBrainsMono',
                    color: CwColors.ink3,
                  ),
                ),
                const SizedBox(width: 4),
                const Icon(Icons.copy, size: 12, color: CwColors.ink4),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
