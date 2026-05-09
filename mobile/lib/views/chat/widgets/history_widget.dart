import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import '../../../theme/colors.dart';

class ChatHistoryWidget extends StatelessWidget {
  final List<dynamic> transactions;
  final int total;

  const ChatHistoryWidget({
    super.key,
    required this.transactions,
    this.total = 0,
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
                '交易记录',
                style: TextStyle(
                  fontSize: 12,
                  fontWeight: FontWeight.w600,
                  color: CwColors.ink3,
                  letterSpacing: 0.5,
                ),
              ),
              const Spacer(),
              Text(
                '共 $total 笔',
                style: const TextStyle(fontSize: 11, color: CwColors.ink4),
              ),
            ],
          ),
          const SizedBox(height: 12),
          if (transactions.isEmpty)
            const Padding(
              padding: EdgeInsets.symmetric(vertical: 16),
              child: Center(
                child: Text(
                  '暂无交易记录',
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
                  '还有 ${transactions.length - 5} 笔交易...',
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
    final token = map['token'] as String? ?? 'ETH';
    final toAddr = map['to_addr'] as String? ?? '';
    final txHash = map['tx_hash'] as String?;
    final createdAt = map['created_at'] as String? ?? '';

    final isSuccess = status == 'confirmed' || status == 'success';
    final isFailed = status == 'failed';

    final shortTo = toAddr.length >= 10
        ? '${toAddr.substring(0, 6)}...${toAddr.substring(toAddr.length - 4)}'
        : toAddr;

    final dateStr = createdAt.length >= 10 ? createdAt.substring(0, 10) : createdAt;

    return GestureDetector(
      onTap: txHash != null
          ? () {
              Clipboard.setData(ClipboardData(text: txHash));
              ScaffoldMessenger.of(context).showSnackBar(
                const SnackBar(content: Text('交易哈希已复制')),
              );
            }
          : null,
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
                    '发送至 $shortTo',
                    style: const TextStyle(fontSize: 13, color: CwColors.ink1),
                  ),
                  const SizedBox(height: 2),
                  Text(
                    dateStr,
                    style: const TextStyle(fontSize: 11, color: CwColors.ink4),
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
                  isSuccess ? '已确认' : (isFailed ? '失败' : '待确认'),
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
}
