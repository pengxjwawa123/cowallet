import 'package:flutter/material.dart';
import '../../../theme/colors.dart';

class ChatTokenInfoWidget extends StatelessWidget {
  final Map<String, dynamic> data;

  const ChatTokenInfoWidget({super.key, required this.data});

  @override
  Widget build(BuildContext context) {
    final token = data['token'] as Map<String, dynamic>? ?? {};
    final balance = data['balance'] as Map<String, dynamic>?;
    final priceUsd = data['price_usd'] as num?;
    final chainId = data['chain_id'] as int?;

    final symbol = token['symbol'] as String? ?? '???';
    final name = token['name'] as String? ?? symbol;
    final tokenType = token['type'] as String? ?? 'ERC-20';
    final decimals = token['decimals'] as int? ?? 18;
    final description = token['description'] as String?;
    final issuer = token['issuer'] as String?;
    final contractAddress = token['contract_address'] as String? ??
        (balance != null ? balance['contract_address'] as String? : null);

    final balanceFormatted = balance?['balance'] as String?;
    final usdValue = balance?['usd_value'] as String?;
    final isNative = balance?['is_native'] as bool? ?? (tokenType == 'native');

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
                width: 36,
                height: 36,
                decoration: BoxDecoration(
                  color: _colorForSymbol(symbol).withValues(alpha: 0.12),
                  borderRadius: BorderRadius.circular(10),
                ),
                child: Center(
                  child: Text(
                    _emojiForSymbol(symbol),
                    style: const TextStyle(fontSize: 18),
                  ),
                ),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      name,
                      style: const TextStyle(
                        fontSize: 16,
                        fontWeight: FontWeight.w700,
                        color: CwColors.ink1,
                      ),
                    ),
                    const SizedBox(height: 2),
                    Row(
                      children: [
                        Text(
                          symbol,
                          style: const TextStyle(
                            fontFamily: 'JetBrainsMono',
                            fontSize: 12,
                            color: CwColors.ink3,
                          ),
                        ),
                        const SizedBox(width: 6),
                        Container(
                          padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 1),
                          decoration: BoxDecoration(
                            color: CwColors.bgSubtle,
                            borderRadius: BorderRadius.circular(4),
                          ),
                          child: Text(
                            tokenType,
                            style: const TextStyle(
                              fontSize: 9,
                              fontWeight: FontWeight.w600,
                              color: CwColors.ink3,
                            ),
                          ),
                        ),
                      ],
                    ),
                  ],
                ),
              ),
              if (priceUsd != null)
                Text(
                  '\$${priceUsd.toStringAsFixed(priceUsd > 100 ? 0 : 2)}',
                  style: const TextStyle(
                    fontFamily: 'JetBrainsMono',
                    fontSize: 18,
                    fontWeight: FontWeight.w700,
                    color: CwColors.ink1,
                  ),
                ),
            ],
          ),

          const SizedBox(height: 14),
          const Divider(height: 1),
          const SizedBox(height: 14),

          // Info rows
          if (balanceFormatted != null)
            _infoRow('Balance', '$balanceFormatted $symbol'),
          if (usdValue != null)
            _infoRow('Value', '\$$usdValue'),
          _infoRow('Decimals', '$decimals'),
          if (chainId != null)
            _infoRow('Chain', _chainName(chainId)),
          if (issuer != null)
            _infoRow('Issuer', issuer),
          if (contractAddress != null && !isNative)
            _infoRow('Contract', _shortenAddress(contractAddress)),

          if (description != null) ...[
            const SizedBox(height: 10),
            Text(
              description,
              style: const TextStyle(
                fontSize: 12,
                color: CwColors.ink3,
                height: 1.5,
              ),
            ),
          ],
        ],
      ),
    );
  }

  Widget _infoRow(String label, String value) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 6),
      child: Row(
        children: [
          Text(
            label,
            style: const TextStyle(
              fontSize: 12,
              color: CwColors.ink3,
            ),
          ),
          const Spacer(),
          Text(
            value,
            style: const TextStyle(
              fontFamily: 'JetBrainsMono',
              fontSize: 12,
              fontWeight: FontWeight.w500,
              color: CwColors.ink2,
            ),
          ),
        ],
      ),
    );
  }

  String _shortenAddress(String addr) {
    if (addr.length < 12) return addr;
    return '${addr.substring(0, 6)}...${addr.substring(addr.length - 4)}';
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

  Color _colorForSymbol(String symbol) {
    switch (symbol) {
      case 'ETH':
      case 'WETH': return const Color(0xFF627EEA);
      case 'USDC': return const Color(0xFF2775CA);
      case 'USDT': return const Color(0xFF26A17B);
      case 'DAI': return const Color(0xFFF5AC37);
      case 'WBTC': return const Color(0xFFF7931A);
      case 'LINK': return const Color(0xFF2A5ADA);
      default: return CwColors.ink3;
    }
  }

  String _emojiForSymbol(String symbol) {
    switch (symbol) {
      case 'ETH':
      case 'WETH': return 'Ξ';
      case 'USDC': return 'Ⓤ';
      case 'USDT': return 'Ⓣ';
      case 'DAI': return 'Ⓓ';
      case 'WBTC': return '₿';
      default: return '🪙';
    }
  }
}
