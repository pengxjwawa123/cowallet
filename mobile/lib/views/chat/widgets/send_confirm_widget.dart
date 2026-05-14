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
  final bool deductGasHint;
  final String? originalAmount;
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
    this.deductGasHint = false,
    this.originalAmount,
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
          color: resolved ? CwColors.line
              : deductGasHint ? CwColors.warn.withValues(alpha: 0.4)
              : CwColors.accent.withValues(alpha: 0.4),
        ),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Icon(
                resolved ? Icons.check_circle
                    : deductGasHint ? Icons.info_outline
                    : Icons.send_rounded,
                size: 16,
                color: resolved ? CwColors.success
                    : deductGasHint ? CwColors.warn
                    : CwColors.accent,
              ),
              const SizedBox(width: 6),
              Text(
                resolved ? '转账已提交'
                    : deductGasHint ? '金额已调整（需预留Gas）'
                    : '转账确认',
                style: TextStyle(
                  fontSize: 12,
                  fontWeight: FontWeight.w600,
                  color: resolved ? CwColors.success
                      : deductGasHint ? CwColors.warn
                      : CwColors.accent,
                  letterSpacing: 0.5,
                ),
              ),
            ],
          ),
          if (deductGasHint && !resolved) ...[
            const SizedBox(height: 12),
            Container(
              width: double.infinity,
              padding: const EdgeInsets.all(12),
              decoration: BoxDecoration(
                color: CwColors.warnSoft,
                borderRadius: BorderRadius.circular(10),
              ),
              child: originalAmount == null
                  ? const Row(
                      mainAxisAlignment: MainAxisAlignment.center,
                      children: [
                        SizedBox(width: 14, height: 14, child: CircularProgressIndicator(strokeWidth: 2, color: CwColors.warn)),
                        SizedBox(width: 8),
                        Text('计算费用中...', style: TextStyle(fontSize: 12, color: CwColors.warn)),
                      ],
                    )
                  : Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        const Text(
                          '转出金额+Gas超出余额，已自动调减',
                          style: TextStyle(fontSize: 12, fontWeight: FontWeight.w600, color: CwColors.warn),
                        ),
                        const SizedBox(height: 8),
                        _breakdownRow('原始金额', '$originalAmount $token'),
                        const SizedBox(height: 4),
                        _breakdownRow('Gas 费用', '- ${gasEstimate ?? "..."} $token'),
                        const Padding(
                          padding: EdgeInsets.symmetric(vertical: 6),
                          child: Divider(height: 1, color: CwColors.lineStrong),
                        ),
                        _breakdownRow('实际转出', '$amount $token', bold: true),
                      ],
                    ),
            ),
          ] else ...[
            const SizedBox(height: 12),
            Text(
              '$amount $token',
              style: const TextStyle(
                fontSize: 24,
                fontWeight: FontWeight.w700,
                color: CwColors.ink1,
              ),
            ),
          ],
          const SizedBox(height: 8),
          _infoRow('收款地址', shortTo),
          const SizedBox(height: 4),
          if (chainId != null)
            _infoRow('网络', _chainName(chainId!)),
          if (chainId != null)
            const SizedBox(height: 4),
          if (!deductGasHint) ...[
            if (gasEstimate != null)
              _infoRow('预估 Gas', gasEstimate!)
            else if (!resolved)
              _gasLoadingRow(),
          ],
          if (!resolved) ...[
            const SizedBox(height: 16),
            Row(
              children: [
                Expanded(
                  child: SizedBox(
                    height: 44,
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
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: SizedBox(
                    height: 44,
                    child: ElevatedButton(
                      onPressed: loading ? null : onConfirm,
                      style: ElevatedButton.styleFrom(
                        backgroundColor: deductGasHint ? CwColors.warn : CwColors.accent,
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
                          : Text(deductGasHint ? '确认转出' : '确认转账'),
                    ),
                  ),
                ),
              ],
            ),
          ],
        ],
      ),
    );
  }

  Widget _breakdownRow(String label, String value, {bool bold = false}) {
    return Row(
      mainAxisAlignment: MainAxisAlignment.spaceBetween,
      children: [
        Text(label, style: TextStyle(
          fontSize: 12,
          color: bold ? CwColors.ink1 : CwColors.ink3,
          fontWeight: bold ? FontWeight.w600 : FontWeight.normal,
        )),
        Text(value, style: TextStyle(
          fontSize: 12,
          fontFamily: 'JetBrainsMono',
          color: bold ? CwColors.ink1 : CwColors.ink2,
          fontWeight: bold ? FontWeight.w600 : FontWeight.normal,
        )),
      ],
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
