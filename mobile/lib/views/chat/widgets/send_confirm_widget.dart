import 'package:flutter/material.dart';
import '../../../theme/colors.dart';
import '../../../l10n/strings.dart';

class ChatSendConfirmWidget extends StatelessWidget {
  final String toAddress;
  final String amount;
  final String token;
  final String? gasEstimate;
  final int? chainId;
  final String? contractAddress;
  final bool loading;
  final bool resolved;
  final bool deductGasHint;
  final String? originalAmount;
  final bool policyRejected;
  final String? policyReason;
  final String? policyLimit;
  final List<String>? policyWarnings;
  final VoidCallback? onConfirm;
  final VoidCallback? onDeny;

  const ChatSendConfirmWidget({
    super.key,
    required this.toAddress,
    required this.amount,
    required this.token,
    this.gasEstimate,
    this.chainId,
    this.contractAddress,
    this.loading = false,
    this.resolved = false,
    this.deductGasHint = false,
    this.originalAmount,
    this.policyRejected = false,
    this.policyReason,
    this.policyLimit,
    this.policyWarnings,
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
          color: policyRejected ? CwColors.danger.withValues(alpha: 0.6)
              : resolved ? CwColors.line
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
                policyRejected ? Icons.block
                    : resolved ? Icons.check_circle
                    : deductGasHint ? Icons.info_outline
                    : Icons.send_rounded,
                size: 16,
                color: policyRejected ? CwColors.danger
                    : resolved ? CwColors.success
                    : deductGasHint ? CwColors.warn
                    : CwColors.accent,
              ),
              const SizedBox(width: 6),
              Text(
                policyRejected ? '转账被拒绝'
                    : resolved ? S.transferSubmitted
                    : deductGasHint ? S.amountAdjustedGas
                    : S.transferConfirm,
                style: TextStyle(
                  fontSize: 12,
                  fontWeight: FontWeight.w600,
                  color: policyRejected ? CwColors.danger
                      : resolved ? CwColors.success
                      : deductGasHint ? CwColors.warn
                      : CwColors.accent,
                  letterSpacing: 0.5,
                ),
              ),
            ],
          ),
          if (policyRejected) ...[
            const SizedBox(height: 12),
            Container(
              width: double.infinity,
              padding: const EdgeInsets.all(12),
              decoration: BoxDecoration(
                color: CwColors.danger.withValues(alpha: 0.08),
                borderRadius: BorderRadius.circular(10),
              ),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    policyReason ?? '超出转账限额',
                    style: const TextStyle(fontSize: 12, fontWeight: FontWeight.w600, color: CwColors.danger),
                  ),
                  if (policyLimit != null) ...[
                    const SizedBox(height: 4),
                    Text(
                      '限额: $policyLimit',
                      style: const TextStyle(fontSize: 11, color: CwColors.ink3),
                    ),
                  ],
                  const SizedBox(height: 8),
                  const Text(
                    '请调整金额或在 设置 > 转账限额 中修改您的限额。',
                    style: TextStyle(fontSize: 11, color: CwColors.ink3),
                  ),
                ],
              ),
            ),
            const SizedBox(height: 8),
            Text(
              '$amount $token',
              style: const TextStyle(
                fontSize: 24,
                fontWeight: FontWeight.w700,
                color: CwColors.ink1,
              ),
            ),
          ] else if (deductGasHint && !resolved) ...[
            const SizedBox(height: 12),
            Container(
              width: double.infinity,
              padding: const EdgeInsets.all(12),
              decoration: BoxDecoration(
                color: CwColors.warnSoft,
                borderRadius: BorderRadius.circular(10),
              ),
              child: originalAmount == null
                  ? Row(
                      mainAxisAlignment: MainAxisAlignment.center,
                      children: [
                        const SizedBox(width: 14, height: 14, child: CircularProgressIndicator(strokeWidth: 2, color: CwColors.warn)),
                        const SizedBox(width: 8),
                        Text(S.calculatingFees, style: const TextStyle(fontSize: 12, color: CwColors.warn)),
                      ],
                    )
                  : Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          S.amountPlusGasExceeded,
                          style: TextStyle(fontSize: 12, fontWeight: FontWeight.w600, color: CwColors.warn),
                        ),
                        const SizedBox(height: 8),
                        _breakdownRow(S.originalAmount, '$originalAmount $token'),
                        const SizedBox(height: 4),
                        _breakdownRow(S.gasFee, '- ${gasEstimate ?? "..."} $token'),
                        const Padding(
                          padding: EdgeInsets.symmetric(vertical: 6),
                          child: Divider(height: 1, color: CwColors.lineStrong),
                        ),
                        _breakdownRow(S.actualSend, '$amount $token', bold: true),
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
          // Policy warnings (non-blocking)
          if (!policyRejected && policyWarnings != null && policyWarnings!.isNotEmpty) ...[
            const SizedBox(height: 8),
            Container(
              width: double.infinity,
              padding: const EdgeInsets.all(10),
              decoration: BoxDecoration(
                color: CwColors.warnSoft,
                borderRadius: BorderRadius.circular(8),
              ),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: policyWarnings!.map((w) => Padding(
                  padding: const EdgeInsets.only(bottom: 4),
                  child: Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      const Icon(Icons.warning_amber_rounded, size: 14, color: CwColors.warn),
                      const SizedBox(width: 6),
                      Expanded(
                        child: Text(w, style: const TextStyle(fontSize: 11, color: CwColors.warn)),
                      ),
                    ],
                  ),
                )).toList(),
              ),
            ),
          ],
          const SizedBox(height: 8),
          _infoRow(S.recipientAddress, shortTo),
          const SizedBox(height: 4),
          if (contractAddress != null && contractAddress!.isNotEmpty) ...[
            _infoRow(S.contract, _shortenAddr(contractAddress!)),
            const SizedBox(height: 4),
          ],
          if (chainId != null)
            _infoRow(S.network, _chainName(chainId!)),
          if (chainId != null)
            const SizedBox(height: 4),
          if (!deductGasHint && !policyRejected) ...[
            if (gasEstimate != null)
              _infoRow(S.estimatedGas, gasEstimate!)
            else if (!resolved)
              _gasLoadingRow(),
          ],
          if (!resolved && !policyRejected) ...[
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
                      child: Text(S.cancel),
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
                          : Text(deductGasHint ? S.confirmSend : S.confirmTransfer),
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

  String _shortenAddr(String addr) {
    if (addr.length < 12) return addr;
    return '${addr.substring(0, 6)}...${addr.substring(addr.length - 4)}';
  }

  Widget _gasLoadingRow() {
    return Row(
      children: [
        Text(S.estimatedGas, style: TextStyle(fontSize: 12, color: CwColors.ink4)),
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
              Text(
                S.estimating,
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
