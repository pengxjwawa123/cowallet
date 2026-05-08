import 'package:flutter/material.dart';
import '../../../theme/colors.dart';

class ChatBalanceWidget extends StatelessWidget {
  final Map<String, dynamic> data;

  const ChatBalanceWidget({super.key, required this.data});

  @override
  Widget build(BuildContext context) {
    final ethBalance = data['eth_balance'] as String? ?? '0';
    final usdcBalance = data['usdc_balance'] as String? ?? '0';
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
                '资产总览',
                style: TextStyle(
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
          _tokenRow('ETH', ethBalance, const Color(0xFF627EEA)),
          const SizedBox(height: 8),
          _tokenRow('USDC', usdcBalance, const Color(0xFF2775CA)),
        ],
      ),
    );
  }

  Widget _tokenRow(String symbol, String balance, Color color) {
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
              symbol[0],
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
        Text(
          balance,
          style: const TextStyle(
            fontSize: 14,
            fontWeight: FontWeight.w500,
            fontFamily: 'JetBrainsMono',
            color: CwColors.ink1,
          ),
        ),
      ],
    );
  }
}
