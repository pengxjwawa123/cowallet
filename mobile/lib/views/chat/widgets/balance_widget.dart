import 'package:flutter/material.dart';
import '../../../theme/colors.dart';

class ChatBalanceWidget extends StatelessWidget {
  final Map<String, dynamic> data;

  const ChatBalanceWidget({super.key, required this.data});

  @override
  Widget build(BuildContext context) {
    final multiChain = data['multi_chain'] as bool? ?? false;
    final totalUsd = data['total_usd'] as String? ?? '—';

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
              const Icon(Icons.account_balance_wallet_outlined,
                  size: 16, color: CwColors.accent),
              const SizedBox(width: 6),
              Text(
                multiChain ? '多链资产总览' : '资产总览',
                style: const TextStyle(
                  fontSize: 12,
                  fontWeight: FontWeight.w600,
                  color: CwColors.ink3,
                  letterSpacing: 0.5,
                ),
              ),
            ],
          ),
          const SizedBox(height: 12),
          Text(
            '\$$totalUsd',
            style: const TextStyle(
              fontSize: 28,
              fontWeight: FontWeight.w700,
              color: CwColors.ink1,
            ),
          ),
          const SizedBox(height: 12),
          if (multiChain)
            _buildMultiChainView()
          else
            _buildSingleChainView(),
        ],
      ),
    );
  }

  Widget _buildMultiChainView() {
    final chains = (data['chains'] as List<dynamic>?) ?? [];
    if (chains.isEmpty) {
      return const Text('暂无资产数据', style: TextStyle(color: CwColors.ink3, fontSize: 13));
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: chains.map<Widget>((chain) {
        final chainData = chain as Map<String, dynamic>;
        final chainName = chainData['chain_name'] as String? ?? '';
        final chainTotalUsd = chainData['total_usd'] as String? ?? '0';
        final tokens = (chainData['tokens'] as List<dynamic>?) ?? [];
        final chainId = chainData['chain_id'] as int? ?? 0;

        if (tokens.isEmpty) return const SizedBox.shrink();

        return Padding(
          padding: const EdgeInsets.only(bottom: 12),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  Container(
                    width: 8,
                    height: 8,
                    decoration: BoxDecoration(
                      color: _chainColor(chainId),
                      shape: BoxShape.circle,
                    ),
                  ),
                  const SizedBox(width: 6),
                  Text(
                    chainName,
                    style: const TextStyle(
                      fontSize: 12,
                      fontWeight: FontWeight.w600,
                      color: CwColors.ink2,
                    ),
                  ),
                  const Spacer(),
                  Text(
                    '\$$chainTotalUsd',
                    style: const TextStyle(
                      fontSize: 12,
                      fontWeight: FontWeight.w500,
                      color: CwColors.ink3,
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 6),
              ...tokens.map<Widget>((t) {
                final token = t as Map<String, dynamic>;
                return Padding(
                  padding: const EdgeInsets.only(left: 14, bottom: 4),
                  child: _tokenRow(
                    token['symbol'] as String? ?? '?',
                    token['balance'] as String? ?? '0',
                    token['usd'] as String? ?? '—',
                  ),
                );
              }),
            ],
          ),
        );
      }).toList(),
    );
  }

  Widget _buildSingleChainView() {
    final tokens = (data['tokens'] as List<dynamic>?) ?? [];
    if (tokens.isEmpty) {
      final ethBalance = data['eth_balance'] as String? ?? '0';
      final usdcBalance = data['usdc_balance'] as String? ?? '0';
      return Column(
        children: [
          _tokenRow('ETH', ethBalance, null),
          const SizedBox(height: 8),
          _tokenRow('USDC', usdcBalance, null),
        ],
      );
    }

    return Column(
      children: tokens.map<Widget>((t) {
        final token = t as Map<String, dynamic>;
        return Padding(
          padding: const EdgeInsets.only(bottom: 6),
          child: _tokenRow(
            token['symbol'] as String? ?? '?',
            token['balance'] as String? ?? '0',
            token['usd'] as String?,
          ),
        );
      }).toList(),
    );
  }

  Widget _tokenRow(String symbol, String balance, String? usd) {
    final color = _tokenColor(symbol);
    return Row(
      children: [
        Container(
          width: 24,
          height: 24,
          decoration: BoxDecoration(
            color: color.withValues(alpha: 0.15),
            shape: BoxShape.circle,
          ),
          child: Center(
            child: Text(
              symbol.isNotEmpty ? symbol[0] : '?',
              style: TextStyle(
                fontSize: 11,
                fontWeight: FontWeight.w700,
                color: color,
              ),
            ),
          ),
        ),
        const SizedBox(width: 10),
        Text(symbol, style: const TextStyle(fontSize: 14, color: CwColors.ink2)),
        const Spacer(),
        Column(
          crossAxisAlignment: CrossAxisAlignment.end,
          children: [
            Text(
              balance,
              style: const TextStyle(
                fontSize: 14,
                fontWeight: FontWeight.w500,
                fontFamily: 'JetBrainsMono',
                color: CwColors.ink1,
              ),
            ),
            if (usd != null && usd != '—' && usd != '0')
              Text(
                '\$$usd',
                style: const TextStyle(fontSize: 11, color: CwColors.ink4),
              ),
          ],
        ),
      ],
    );
  }

  static Color _tokenColor(String symbol) {
    switch (symbol.toUpperCase()) {
      case 'ETH':
      case 'WETH':
        return const Color(0xFF627EEA);
      case 'USDC':
        return const Color(0xFF2775CA);
      case 'USDT':
        return const Color(0xFF26A17B);
      case 'DAI':
        return const Color(0xFFF5AC37);
      case 'BNB':
        return const Color(0xFFF3BA2F);
      case 'POL':
      case 'MATIC':
        return const Color(0xFF8247E5);
      default:
        return CwColors.ink3;
    }
  }

  static Color _chainColor(int chainId) {
    switch (chainId) {
      case 1:
        return const Color(0xFF627EEA);
      case 8453:
        return const Color(0xFF0052FF);
      case 42161:
        return const Color(0xFF28A0F0);
      case 10:
        return const Color(0xFFFF0420);
      case 56:
        return const Color(0xFFF3BA2F);
      case 137:
        return const Color(0xFF8247E5);
      default:
        return CwColors.ink3;
    }
  }
}
