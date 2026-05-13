import 'package:flutter/material.dart';
import '../../../theme/colors.dart';

class ChatSendConfirmWidget extends StatelessWidget {
  final String toAddress;
  final String amount;
  final String token;
  final String? gasEstimate;
  final int? chainId;
  final bool loading;
  final bool resolved;
  final VoidCallback? onConfirm;
  final VoidCallback? onDeny;

  const ChatSendConfirmWidget({
    super.key,
    required this.toAddress,
    required this.amount,
    required this.token,
    this.gasEstimate,
    this.chainId,
    this.loading = false,
    this.resolved = false,
    this.onConfirm,
    this.onDeny,
  });

  @override
  Widget build(BuildContext context) {
    final shortTo = toAddress.length >= 10
        ? '${toAddress.substring(0, 6)}...${toAddress.substring(toAddress.length - 4)}'
        : toAddress;

    return Container(
      margin: const EdgeInsets.only(bottom: 12),
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: CwColors.bgCard,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(
          color: resolved ? CwColors.line : CwColors.accent.withValues(alpha: 0.4),
        ),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Icon(
                resolved ? Icons.check_circle : Icons.send_rounded,
                size: 16,
                color: resolved ? CwColors.success : CwColors.accent,
              ),
              const SizedBox(width: 6),
              Text(
                resolved ? '转账已提交' : '转账确认',
                style: TextStyle(
                  fontSize: 12,
                  fontWeight: FontWeight.w600,
                  color: resolved ? CwColors.success : CwColors.accent,
                  letterSpacing: 0.5,
                ),
              ),
            ],
          ),
          const SizedBox(height: 12),
          Text(
            '$amount $token',
            style: const TextStyle(
              fontSize: 24,
              fontWeight: FontWeight.w700,
              color: CwColors.ink1,
            ),
          ),
          const SizedBox(height: 8),
          _infoRow('收款地址', shortTo),
          const SizedBox(height: 4),
          if (chainId != null)
            _infoRow('网络', _chainName(chainId!)),
          if (chainId != null)
            const SizedBox(height: 4),
          if (gasEstimate != null)
            _infoRow('预估 Gas', gasEstimate!)
          else if (!resolved)
            _gasLoadingRow(),
          if (!resolved) ...[
            const SizedBox(height: 16),
            Row(
              children: [
                Expanded(
                  child: OutlinedButton(
                    onPressed: loading ? null : onDeny,
                    style: OutlinedButton.styleFrom(
                      foregroundColor: CwColors.ink3,
                      side: const BorderSide(color: CwColors.line),
                      shape: RoundedRectangleBorder(
                        borderRadius: BorderRadius.circular(10),
                      ),
                    ),
                    child: const Text('取消'),
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: ElevatedButton(
                    onPressed: loading ? null : onConfirm,
                    style: ElevatedButton.styleFrom(
                      backgroundColor: CwColors.accent,
                      foregroundColor: Colors.white,
                      shape: RoundedRectangleBorder(
                        borderRadius: BorderRadius.circular(10),
                      ),
                    ),
                    child: loading
                        ? const SizedBox(
                            width: 16,
                            height: 16,
                            child: CircularProgressIndicator(
                              strokeWidth: 2,
                              color: Colors.white,
                            ),
                          )
                        : const Text('确认转账'),
                  ),
                ),
              ],
            ),
          ],
        ],
      ),
    );
  }

  Widget _infoRow(String label, String value) {
    return Row(
      children: [
        Text(label, style: const TextStyle(fontSize: 12, color: CwColors.ink4)),
        const SizedBox(width: 8),
        Expanded(
          child: Text(
            value,
            style: const TextStyle(
              fontSize: 12,
              fontFamily: 'JetBrainsMono',
              color: CwColors.ink2,
            ),
            textAlign: TextAlign.right,
          ),
        ),
      ],
    );
  }

  String _chainName(int chainId) {
    switch (chainId) {
      case 1: return 'Ethereum';
      case 8453: return 'Base';
      case 42161: return 'Arbitrum';
      case 10: return 'Optimism';
      case 56: return 'BNB Chain';
      case 137: return 'Polygon';
      default: return 'Chain $chainId';
    }
  }

  Widget _gasLoadingRow() {
    return Row(
      children: [
        const Text('预估 Gas', style: TextStyle(fontSize: 12, color: CwColors.ink4)),
        const SizedBox(width: 8),
        Expanded(
          child: Row(
            mainAxisAlignment: MainAxisAlignment.end,
            children: [
              SizedBox(
                width: 10,
                height: 10,
                child: CircularProgressIndicator(
                  strokeWidth: 1.5,
                  color: CwColors.ink4,
                ),
              ),
              const SizedBox(width: 6),
              const Text(
                '估算中...',
                style: TextStyle(
                  fontSize: 12,
                  color: CwColors.ink4,
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }
}
