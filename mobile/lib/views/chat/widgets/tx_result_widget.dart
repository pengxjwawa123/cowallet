import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import '../../../theme/colors.dart';

class ChatTxResultWidget extends StatelessWidget {
  final String txHash;
  final bool success;
  final String? amount;
  final String? token;

  const ChatTxResultWidget({
    super.key,
    required this.txHash,
    this.success = true,
    this.amount,
    this.token,
  });

  @override
  Widget build(BuildContext context) {
    final shortHash = txHash.length >= 16
        ? '${txHash.substring(0, 10)}...${txHash.substring(txHash.length - 6)}'
        : txHash;

    return Container(
      margin: const EdgeInsets.only(bottom: 12),
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: success
            ? CwColors.success.withValues(alpha: 0.06)
            : CwColors.danger.withValues(alpha: 0.06),
        borderRadius: BorderRadius.circular(16),
        border: Border.all(
          color: success
              ? CwColors.success.withValues(alpha: 0.3)
              : CwColors.danger.withValues(alpha: 0.3),
        ),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Icon(
                success ? Icons.check_circle : Icons.error,
                size: 18,
                color: success ? CwColors.success : CwColors.danger,
              ),
              const SizedBox(width: 8),
              Text(
                success ? '交易已广播' : '交易失败',
                style: TextStyle(
                  fontSize: 14,
                  fontWeight: FontWeight.w600,
                  color: success ? CwColors.success : CwColors.danger,
                ),
              ),
            ],
          ),
          if (amount != null && token != null) ...[
            const SizedBox(height: 8),
            Text(
              '$amount $token',
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
              Clipboard.setData(ClipboardData(text: txHash));
              ScaffoldMessenger.of(context).showSnackBar(
                const SnackBar(content: Text('交易哈希已复制')),
              );
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
