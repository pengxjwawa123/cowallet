import 'package:flutter/foundation.dart';

import 'chain_service.dart';

class BalanceService extends ChangeNotifier {
  final ChainService _chain;

  BigInt ethBalance = BigInt.zero;
  BigInt usdcBalance = BigInt.zero;
  bool loading = false;
  String? error;
  DateTime? lastUpdated;

  BalanceService(this._chain);

  Future<void> refresh(String address) async {
    loading = true;
    error = null;
    notifyListeners();

    try {
      final results = await Future.wait([
        _chain.getEthBalance(address),
        _chain.getTokenBalance(address, ChainConfig.baseSepolia.usdcContract),
      ]);
      ethBalance = results[0];
      usdcBalance = results[1];
      lastUpdated = DateTime.now();
    } catch (e) {
      error = e.toString();
    } finally {
      loading = false;
      notifyListeners();
    }
  }

  String get formattedEth {
    final whole = ethBalance ~/ BigInt.from(10).pow(18);
    final frac = (ethBalance % BigInt.from(10).pow(18))
        .toString()
        .padLeft(18, '0')
        .substring(0, 4);
    return '$whole.$frac ETH';
  }

  String get formattedUsdc {
    final whole = usdcBalance ~/ BigInt.from(10).pow(6);
    final frac = (usdcBalance % BigInt.from(10).pow(6))
        .toString()
        .padLeft(6, '0')
        .substring(0, 2);
    final wholeStr = _addCommas(whole.toString());
    return '$wholeStr.$frac USDC';
  }

  /// Placeholder price: ETH at \$3200, USDC at \$1
  String get formattedTotal {
    final ethInUsd = ethBalance * BigInt.from(3200) ~/ BigInt.from(10).pow(18);
    final usdcInUsd = usdcBalance ~/ BigInt.from(10).pow(6);
    final total = ethInUsd + usdcInUsd;
    return '\$${_addCommas(total.toString())}';
  }

  static String _addCommas(String number) {
    final buf = StringBuffer();
    final len = number.length;
    for (var i = 0; i < len; i++) {
      if (i > 0 && (len - i) % 3 == 0) buf.write(',');
      buf.write(number[i]);
    }
    return buf.toString();
  }
}
