import 'package:flutter/material.dart';
import '../../../theme/colors.dart';
import '../../../l10n/strings.dart';

class ChatSwapConfirmWidget extends StatelessWidget {
  final String fromToken;
  final String toToken;
  final String amount;
  final String estimatedOutput;
  final double slippage;
  final bool loading;
  final bool resolved;
  final VoidCallback? onConfirm;
  final VoidCallback? onDeny;
  final int? chainId;

  const ChatSwapConfirmWidget({
    super.key,
    required this.fromToken,
    required this.toToken,
    required this.amount,
    required this.estimatedOutput,
    this.slippage = 0.5,
    this.loading = false,
    this.resolved = false,
    this.onConfirm,
    this.onDeny,
    this.chainId,
  });

  @override
  Widget build(BuildContext context) {
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
                resolved ? Icons.check_circle : Icons.swap_horiz,
                size: 16,
                color: resolved ? CwColors.success : CwColors.accent,
              ),
              const SizedBox(width: 6),
              Text(
                resolved ? S.swapSubmitted : S.swapConfirm,
                style: TextStyle(
                  fontSize: 12,
                  fontWeight: FontWeight.w600,
                  color: resolved ? CwColors.success : CwColors.accent,
                  letterSpacing: 0.5,
                ),
              ),
            ],
          ),
          const SizedBox(height: 16),
          // Swap visualization
          Row(
            children: [
              Expanded(
                child: _tokenBox(fromToken, amount, S.pay),
              ),
              Padding(
                padding: const EdgeInsets.symmetric(horizontal: 8),
                child: Icon(
                  Icons.arrow_forward_rounded,
                  size: 20,
                  color: CwColors.ink3,
                ),
              ),
              Expanded(
                child: _tokenBox(toToken, estimatedOutput, S.estimatedReceive),
              ),
            ],
          ),
          const SizedBox(height: 12),
          _infoRow(S.slippageTolerance, '${slippage}%'),
          if (chainId != null)
            _infoRow(S.network, _chainName(chainId!)),
          _infoRow(S.route, '$fromToken → $toToken'),
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
                    child: Text(S.cancel),
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
                        : Text(S.confirmSwap),
                  ),
                ),
              ],
            ),
          ],
        ],
      ),
    );
  }

  Widget _tokenBox(String token, String amount, String label) {
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: CwColors.bgPaper,
        borderRadius: BorderRadius.circular(10),
        border: Border.all(color: CwColors.line),
      ),
      child: Column(
        children: [
          Text(
            label,
            style: const TextStyle(fontSize: 10, color: CwColors.ink4),
          ),
          const SizedBox(height: 4),
          Text(
            amount,
            style: const TextStyle(
              fontSize: 16,
              fontWeight: FontWeight.w600,
              fontFamily: 'JetBrainsMono',
              color: CwColors.ink1,
            ),
          ),
          Text(
            token,
            style: const TextStyle(fontSize: 12, color: CwColors.ink3),
          ),
        ],
      ),
    );
  }

  Widget _infoRow(String label, String value) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 4),
      child: Row(
        children: [
          Text(label, style: const TextStyle(fontSize: 12, color: CwColors.ink4)),
          const Spacer(),
          Text(
            value,
            style: const TextStyle(fontSize: 12, color: CwColors.ink2),
          ),
        ],
      ),
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
}
