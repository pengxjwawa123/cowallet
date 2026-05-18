import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import '../../../theme/colors.dart';
import '../../../widgets/top_toast.dart';
import '../../../l10n/strings.dart';

class ChatHistoryWidget extends StatelessWidget {
  final List<dynamic> transactions;
  final int total;
  final ValueChanged<Map<String, dynamic>>? onTxTap;

  const ChatHistoryWidget({
    super.key,
    required this.transactions,
    this.total = 0,
    this.onTxTap,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      margin: const EdgeInsets.only(bottom: 12),
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: CwColors.bgCard,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: CwColors.line),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              const Icon(Icons.receipt_long, size: 16, color: CwColors.accent),
              const SizedBox(width: 6),
              Text(
                S.txHistory,
                style: TextStyle(
                  fontSize: 12,
                  fontWeight: FontWeight.w600,
                  color: CwColors.ink3,
                  letterSpacing: 0.5,
                ),
              ),
              const Spacer(),
              Text(
                S.txCount(total),
                style: const TextStyle(fontSize: 11, color: CwColors.ink4),
              ),
            ],
          ),
          const SizedBox(height: 12),
          if (transactions.isEmpty)
            Padding(
              padding: EdgeInsets.symmetric(vertical: 16),
              child: Center(
                child: Text(
                  S.noTxHistory,
                  style: TextStyle(fontSize: 13, color: CwColors.ink4),
                ),
              ),
            )
          else
            ...transactions.take(5).map((tx) => _buildTxRow(context, tx)).toList(),
          if (transactions.length > 5)
            Padding(
              padding: const EdgeInsets.only(top: 8),
              child: Center(
                child: Text(
                  S.moreTxCount(transactions.length - 5),
                  style: const TextStyle(fontSize: 11, color: CwColors.ink4),
                ),
              ),
            ),
        ],
      ),
    );
  }

  Widget _buildTxRow(BuildContext context, dynamic tx) {
    final map = tx is Map<String, dynamic> ? tx : <String, dynamic>{};
    final status = map['status'] as String? ?? 'unknown';
    final value = map['value'] as String? ?? '0';
    final token = (map['token'] ?? map['token_symbol'] ?? 'ETH') as String;
    final toAddr = (map['to_addr'] ?? map['to'] ?? '') as String;
    final txHash = map['tx_hash'] as String?;
    final chainName = map['chain_name'] as String?;
    final createdAt = (map['created_at'] ?? map['timestamp'] ?? '') as String;

    final isSuccess = status == 'confirmed' || status == 'success';
    final isFailed = status == 'failed';

    final shortTo = toAddr.length >= 10
        ? '${toAddr.substring(0, 6)}...${toAddr.substring(toAddr.length - 4)}'
        : toAddr;

    final dateStr = _formatDate(createdAt);

    return GestureDetector(
      onTap: () {
        if (onTxTap != null) {
          onTxTap!(map);
        } else if (txHash != null) {
          Clipboard.setData(ClipboardData(text: txHash));
          showTopToast(context, S.txHashCopied);
        }
      },
      child: Container(
        padding: const EdgeInsets.symmetric(vertical: 10),
        decoration: const BoxDecoration(
          border: Border(bottom: BorderSide(color: CwColors.line, width: 0.5)),
        ),
        child: Row(
          children: [
            Container(
              width: 32,
              height: 32,
              decoration: BoxDecoration(
                color: isFailed
                    ? CwColors.danger.withValues(alpha: 0.1)
                    : CwColors.accent.withValues(alpha: 0.1),
                shape: BoxShape.circle,
              ),
              child: Icon(
                isFailed
                    ? Icons.close
                    : Icons.arrow_upward_rounded,
                size: 14,
                color: isFailed ? CwColors.danger : CwColors.accent,
              ),
            ),
            const SizedBox(width: 10),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    S.sendTo(shortTo),
                    style: const TextStyle(fontSize: 13, color: CwColors.ink1),
                  ),
                  const SizedBox(height: 2),
                  Row(
                    children: [
                      if (chainName != null) ...[
                        Text(
                          chainName,
                          style: const TextStyle(fontSize: 11, color: CwColors.accent, fontWeight: FontWeight.w500),
                        ),
                        const Text(' · ', style: TextStyle(fontSize: 11, color: CwColors.ink4)),
                      ],
                      Text(
                        dateStr,
                        style: const TextStyle(fontSize: 11, color: CwColors.ink4),
                      ),
                    ],
                  ),
                ],
              ),
            ),
            Column(
              crossAxisAlignment: CrossAxisAlignment.end,
              children: [
                Text(
                  '-$value $token',
                  style: TextStyle(
                    fontSize: 13,
                    fontFamily: 'JetBrainsMono',
                    fontWeight: FontWeight.w500,
                    color: isFailed ? CwColors.danger : CwColors.ink1,
                  ),
                ),
                Text(
                  isSuccess ? S.confirmed : (isFailed ? S.failed : S.pending),
                  style: TextStyle(
                    fontSize: 10,
                    color: isSuccess
                        ? CwColors.success
                        : (isFailed ? CwColors.danger : CwColors.ink4),
                  ),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  String _formatDate(String raw) {
    if (raw.isEmpty) return '';
    try {
      final dt = DateTime.parse(raw);
      return '${dt.month}/${dt.day} ${dt.hour.toString().padLeft(2, '0')}:${dt.minute.toString().padLeft(2, '0')}';
    } catch (_) {
      return raw.length >= 10 ? raw.substring(0, 10) : raw;
    }
  }
}
