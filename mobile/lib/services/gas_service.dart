import 'chain_service.dart';

class GasEstimate {
  final BigInt gasLimit;
  final BigInt maxFeePerGas;
  final BigInt maxPriorityFeePerGas;
  final BigInt totalWei;
  final String formattedEth;
  final String formattedUsd;

  const GasEstimate({
    required this.gasLimit,
    required this.maxFeePerGas,
    required this.maxPriorityFeePerGas,
    required this.totalWei,
    required this.formattedEth,
    required this.formattedUsd,
  });
}

class GasService {
  final ChainService _chain;

  BigInt? _cachedGasPrice;
  DateTime? _cacheTime;
  static const _cacheDuration = Duration(seconds: 15);
  static const int _ethPriceUsd = 3200;

  GasService(this._chain);

  /// Invalidate cached gas price (e.g. after chain switch).
  void clearCache() {
    _cachedGasPrice = null;
    _cacheTime = null;
  }

  Future<BigInt> _getGasPrice() async {
    if (_cachedGasPrice != null &&
        _cacheTime != null &&
        DateTime.now().difference(_cacheTime!) < _cacheDuration) {
      return _cachedGasPrice!;
    }
    _cachedGasPrice = await _chain.getGasPrice();
    _cacheTime = DateTime.now();
    return _cachedGasPrice!;
  }

  Future<GasEstimate> estimate({
    required String from,
    required String to,
    required BigInt value,
    String? data,
  }) async {
    final hasCalldata = data != null && data.isNotEmpty;

    final results = await Future.wait([
      _getGasPrice(),
      if (hasCalldata)
        _chain.estimateGas({
          'from': from,
          'to': to,
          'value': '0x${value.toRadixString(16)}',
          'data': data,
        })
      else
        Future.value(BigInt.from(21000)),
    ]);

    final gasPrice = results[0];
    final gasLimit = results[1];
    final maxFee = gasPrice * BigInt.two;
    final maxPriority = BigInt.from(1500000000); // 1.5 gwei
    final totalWei = gasLimit * maxFee;

    final ethWhole = totalWei ~/ BigInt.from(10).pow(18);
    final ethFrac = (totalWei % BigInt.from(10).pow(18))
        .toString()
        .padLeft(18, '0')
        .substring(0, 6);
    final formattedEth = '$ethWhole.$ethFrac ETH';

    final usdCents =
        totalWei * BigInt.from(_ethPriceUsd * 100) ~/ BigInt.from(10).pow(18);
    final usdWhole = usdCents ~/ BigInt.from(100);
    final usdFrac = (usdCents % BigInt.from(100)).toString().padLeft(2, '0');
    final formattedUsd = '\$$usdWhole.$usdFrac';

    return GasEstimate(
      gasLimit: gasLimit,
      maxFeePerGas: maxFee,
      maxPriorityFeePerGas: maxPriority,
      totalWei: totalWei,
      formattedEth: formattedEth,
      formattedUsd: formattedUsd,
    );
  }
}
