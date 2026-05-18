import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import '../../../theme/colors.dart';
import '../../../widgets/top_toast.dart';
import '../../../l10n/strings.dart';

class ChatTxDetailWidget extends StatelessWidget {
  final Map<String, dynamic> data;

  const ChatTxDetailWidget({super.key, required this.data});

  @override
  Widget build(BuildContext context) {
    final txHash = data['tx_hash'] as String? ?? '';
    final from = (data['from'] ?? data['from_addr'] ?? '') as String;
    final to = (data['to'] ?? data['to_addr'] ?? '') as String;
    final value = data['value'] as String? ?? '0';
    final token = (data['token'] ?? data['token_symbol'] ?? 'ETH') as String;
    final status = data['status'] as String? ?? '';
    final chainId = data['chain_id'] as int? ?? 1;
    final blockNumber = data['block_number'] as int?;
    final timestamp = (data['timestamp'] ?? data['created_at']) as String?;
    final gasUsed = data['gas_used']?.toString();
    final isIncoming = data['is_incoming'] == true;

    final isSuccess = status == 'confirmed' || status == 'success';
    final isFailed = status == 'failed';
    final isPending = status == 'pending' || status == 'broadcast';

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
          // Header
          Row(
            children: [
              Container(
                width: 32,
                height: 32,
                decoration: BoxDecoration(
                  color: isFailed
                      ? CwColors.danger.withValues(alpha: 0.1)
                      : isIncoming
                          ? CwColors.success.withValues(alpha: 0.1)
                          : CwColors.accent.withValues(alpha: 0.1),
                  shape: BoxShape.circle,
                ),
                child: Icon(
                  isFailed
                      ? Icons.close
                      : isIncoming
                          ? Icons.arrow_downward_rounded
                          : Icons.arrow_upward_rounded,
                  size: 16,
                  color: isFailed
                      ? CwColors.danger
                      : isIncoming
                          ? CwColors.success
                          : CwColors.accent,
                ),
              ),
              const SizedBox(width: 10),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      isIncoming ? S.receive : S.transfer,
                      style: const TextStyle(
                        fontSize: 14,
                        fontWeight: FontWeight.w600,
                        color: CwColors.ink1,
                      ),
                    ),
                    const SizedBox(height: 2),
                    Text(
                      _chainName(chainId),
                      style: TextStyle(
                        fontSize: 11,
                        color: _chainColor(chainId),
                        fontWeight: FontWeight.w500,
                      ),
                    ),
                  ],
                ),
              ),
              _statusBadge(isSuccess, isFailed, isPending),
            ],
          ),

          const SizedBox(height: 14),

          // Amount
          Text(
            '${isIncoming ? "+" : "-"}${_formatValue(value, token)} $token',
            style: TextStyle(
              fontSize: 22,
              fontWeight: FontWeight.w700,
              fontFamily: 'JetBrainsMono',
              color: isIncoming ? CwColors.success : CwColors.ink1,
            ),
          ),

          const SizedBox(height: 14),
          const Divider(height: 1, color: CwColors.line),
          const SizedBox(height: 14),

          // Details
          _detailRow(S.sender, _shortAddr(from), from),
          const SizedBox(height: 10),
          _detailRow(S.receiver, _shortAddr(to), to),
          const SizedBox(height: 10),
          _detailRow(S.network, _chainName(chainId), null),
          if (blockNumber != null) ...[
            const SizedBox(height: 10),
            _detailRow(S.block, '#$blockNumber', null),
          ],
          if (gasUsed != null) ...[
            const SizedBox(height: 10),
            _detailRow('Gas', gasUsed, null),
          ],
          if (timestamp != null) ...[
            const SizedBox(height: 10),
            _detailRow(S.time, _formatTime(timestamp), null),
          ],

          if (txHash.isNotEmpty) ...[
            const SizedBox(height: 14),
            const Divider(height: 1, color: CwColors.line),
            const SizedBox(height: 12),
            // Tx Hash row
            Builder(builder: (ctx) {
              final shortHash = txHash.length >= 16
                  ? '${txHash.substring(0, 10)}...${txHash.substring(txHash.length - 6)}'
                  : txHash;
              return GestureDetector(
                onTap: () {
                  Clipboard.setData(ClipboardData(text: txHash));
                  showTopToast(ctx, S.txHashCopied);
                },
                child: Row(
                  children: [
                    const Text(
                      'Tx Hash',
                      style: TextStyle(fontSize: 12, color: CwColors.ink4),
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        shortHash,
                        style: const TextStyle(
                          fontSize: 12,
                          fontFamily: 'JetBrainsMono',
                          color: CwColors.accent,
                        ),
                        textAlign: TextAlign.right,
                      ),
                    ),
                    const SizedBox(width: 4),
                    const Icon(Icons.copy, size: 12, color: CwColors.ink4),
                  ],
                ),
              );
            }),
          ],
        ],
      ),
    );
  }

  Widget _detailRow(String label, String value, String? copyValue) {
    return Builder(builder: (ctx) {
      return GestureDetector(
        onTap: copyValue != null
            ? () {
                Clipboard.setData(ClipboardData(text: copyValue));
                showTopToast(ctx, S.labelCopied(label));
              }
            : null,
        child: Row(
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
            if (copyValue != null) ...[
              const SizedBox(width: 4),
              const Icon(Icons.copy, size: 10, color: CwColors.ink4),
            ],
          ],
        ),
      );
    });
  }

  Widget _statusBadge(bool isSuccess, bool isFailed, bool isPending) {
    final Color color;
    final String label;
    if (isSuccess) {
      color = CwColors.success;
      label = S.confirmed;
    } else if (isFailed) {
      color = CwColors.danger;
      label = S.failed;
    } else if (isPending) {
      color = CwColors.warn;
      label = S.pending;
    } else {
      color = CwColors.ink4;
      label = S.unknown;
    }

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.1),
        borderRadius: BorderRadius.circular(6),
      ),
      child: Text(
        label,
        style: TextStyle(fontSize: 11, fontWeight: FontWeight.w600, color: color),
      ),
    );
  }

  String _shortAddr(String addr) {
    if (addr.length < 10) return addr;
    return '${addr.substring(0, 6)}...${addr.substring(addr.length - 4)}';
  }

  String _formatValue(String value, String token) {
    try {
      final val = BigInt.parse(value);
      final decimals = (token == 'USDC' || token == 'USDT') ? 6 : 18;
      final divisor = BigInt.from(10).pow(decimals);
      final whole = val ~/ divisor;
      final frac = val.remainder(divisor).abs();
      final fracStr = frac.toString().padLeft(decimals, '0');
      final trimmed = fracStr.substring(0, 6).replaceAll(RegExp(r'0+$'), '');
      if (trimmed.isEmpty) return whole.toString();
      return '$whole.$trimmed';
    } catch (_) {
      return value;
    }
  }

  String _formatTime(String timestamp) {
    try {
      final dt = DateTime.parse(timestamp);
      return '${dt.year}-${dt.month.toString().padLeft(2, '0')}-${dt.day.toString().padLeft(2, '0')} ${dt.hour.toString().padLeft(2, '0')}:${dt.minute.toString().padLeft(2, '0')}';
    } catch (_) {
      return timestamp;
    }
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

  Color _chainColor(int chainId) {
    switch (chainId) {
      case 1: return const Color(0xFF627EEA);
      case 8453: return const Color(0xFF0052FF);
      case 42161: return const Color(0xFF28A0F0);
      case 10: return const Color(0xFFFF0420);
      case 56: return const Color(0xFFF3BA2F);
      case 137: return const Color(0xFF8247E5);
      default: return CwColors.ink3;
    }
  }
}
