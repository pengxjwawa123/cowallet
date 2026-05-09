import 'package:flutter/foundation.dart';
import '../api/balance_api.dart';

class TokenBalance {
  final String symbol;
  final String balance;
  final String usd;
  final bool native;

  TokenBalance({
    required this.symbol,
    required this.balance,
    required this.usd,
    required this.native,
  });

  factory TokenBalance.fromJson(Map<String, dynamic> json) {
    return TokenBalance(
      symbol: json['symbol'] as String? ?? '???',
      balance: json['balance'] as String? ?? '0',
      usd: json['usd'] as String? ?? '0.00',
      native: json['native'] as bool? ?? false,
    );
  }
}

class BalanceService extends ChangeNotifier {
  // Multi-chain state
  Map<int, List<TokenBalance>> _chainTokens = {};
  Map<int, String> _chainTotals = {};
  String _portfolioTotalUsd = '0.00';

  // Legacy single-chain state (for backward compatibility)
  List<TokenBalance> tokens = [];
  String totalUsd = '0.00';

  bool loading = false;
  String? error;
  DateTime? lastUpdated;

  // Multi-chain getters
  String get portfolioTotalUsd => _portfolioTotalUsd;
  Map<int, String> get chainTotals => Map.unmodifiable(_chainTotals);

  List<TokenBalance> get allTokens {
    final all = <TokenBalance>[];
    for (final tokens in _chainTokens.values) {
      all.addAll(tokens);
    }
    // Sort by USD value descending
    all.sort((a, b) {
      final aUsd = double.tryParse(a.usd) ?? 0.0;
      final bUsd = double.tryParse(b.usd) ?? 0.0;
      return bUsd.compareTo(aUsd);
    });
    return all;
  }

  List<TokenBalance> tokensForChain(int chainId) {
    return _chainTokens[chainId] ?? [];
  }

  /// Refresh all chains balances (multi-chain)
  Future<void> refresh(String address, {int? chainId}) async {
    if (address.isEmpty) {
      error = 'No wallet address';
      notifyListeners();
      return;
    }

    loading = true;
    error = null;
    notifyListeners();

    try {
      // If chainId is provided, use single-chain API for backward compatibility
      if (chainId != null) {
        await refreshSingleChain(address, chainId: chainId);
        return;
      }

      // Otherwise use multi-chain API
      final result = await BalanceApi.getAllBalances(address: address);

      if (result.isSuccess && result.data != null) {
        final data = result.data!;
        _portfolioTotalUsd = data['total_usd'] as String? ?? '0.00';

        final chainsList = data['chains'] as List<dynamic>? ?? [];
        _chainTokens.clear();
        _chainTotals.clear();

        for (final chainData in chainsList) {
          final chainMap = chainData as Map<String, dynamic>;
          final chainId = chainMap['chain_id'] as int;
          final chainTotal = chainMap['total_usd'] as String? ?? '0.00';
          final tokensList = chainMap['tokens'] as List<dynamic>? ?? [];

          _chainTotals[chainId] = chainTotal;
          _chainTokens[chainId] = tokensList
              .map((json) => TokenBalance.fromJson(json as Map<String, dynamic>))
              .toList();
        }

        // Update legacy fields with aggregated data
        totalUsd = _portfolioTotalUsd;
        tokens = allTokens;

        lastUpdated = DateTime.now();
        error = null;
      } else {
        error = result.errorMessage ?? 'Failed to load balance';
      }
    } catch (e) {
      error = e.toString();
    } finally {
      loading = false;
      notifyListeners();
    }
  }

  /// Refresh single chain balance (legacy/fallback)
  Future<void> refreshSingleChain(String address, {required int chainId}) async {
    if (address.isEmpty) {
      error = 'No wallet address';
      notifyListeners();
      return;
    }

    loading = true;
    error = null;
    notifyListeners();

    try {
      final result = await BalanceApi.getBalance(
        address: address,
        chainId: chainId,
      );

      if (result.isSuccess && result.data != null) {
        final data = result.data!;
        final chainTotal = data['total_usd'] as String? ?? '0.00';

        final tokensList = data['tokens'] as List<dynamic>? ?? [];
        final chainTokens = tokensList
            .map((json) => TokenBalance.fromJson(json as Map<String, dynamic>))
            .toList();

        // Update chain-specific data
        _chainTokens[chainId] = chainTokens;
        _chainTotals[chainId] = chainTotal;

        // Recalculate portfolio total
        double total = 0.0;
        for (final chainTotal in _chainTotals.values) {
          total += double.tryParse(chainTotal) ?? 0.0;
        }
        _portfolioTotalUsd = total.toStringAsFixed(2);

        // Update legacy fields
        totalUsd = chainTotal;
        tokens = chainTokens;

        lastUpdated = DateTime.now();
        error = null;
      } else {
        error = result.errorMessage ?? 'Failed to load balance';
      }
    } catch (e) {
      error = e.toString();
    } finally {
      loading = false;
      notifyListeners();
    }
  }

  // Helper getters for backward compatibility and convenience
  String get formattedTotal {
    if (totalUsd == '0.00' || totalUsd.isEmpty) return '\$0';
    final amount = double.tryParse(totalUsd) ?? 0.0;
    return '\$${_formatNumber(amount)}';
  }

  String get formattedEth {
    final ethToken = tokens.firstWhere(
      (t) => t.symbol == 'ETH',
      orElse: () => TokenBalance(symbol: 'ETH', balance: '0', usd: '0.00', native: true),
    );
    return '${ethToken.balance} ETH';
  }

  String get formattedUsdc {
    final usdcToken = tokens.firstWhere(
      (t) => t.symbol == 'USDC',
      orElse: () => TokenBalance(symbol: 'USDC', balance: '0', usd: '0.00', native: false),
    );
    return '${usdcToken.balance} USDC';
  }

  List<TokenBalance> get topTokens {
    // Return top 3 tokens (native first, then by USD value)
    final sorted = List<TokenBalance>.from(tokens);
    sorted.sort((a, b) {
      if (a.native != b.native) return a.native ? -1 : 1;
      final aUsd = double.tryParse(a.usd) ?? 0.0;
      final bUsd = double.tryParse(b.usd) ?? 0.0;
      return bUsd.compareTo(aUsd);
    });
    return sorted.take(3).toList();
  }

  static String _formatNumber(double value) {
    if (value >= 1000000) {
      return '${(value / 1000000).toStringAsFixed(2)}M';
    } else if (value >= 1000) {
      return '${(value / 1000).toStringAsFixed(2)}K';
    } else if (value >= 100) {
      return value.toStringAsFixed(0);
    } else if (value >= 1) {
      return value.toStringAsFixed(2);
    } else {
      return value.toStringAsFixed(2);
    }
  }
}
