import 'package:dio/dio.dart';

class ChainConfig {
  final int chainId;
  final String name;
  final String rpcUrl;
  final String usdcContract;
  final String usdtContract;
  final String daiContract;
  final String wethContract;

  const ChainConfig({
    required this.chainId,
    required this.name,
    required this.rpcUrl,
    required this.usdcContract,
    this.usdtContract = '',
    this.daiContract = '',
    this.wethContract = '',
  });

  static const ethereum = ChainConfig(
    chainId: 1,
    name: 'Ethereum',
    rpcUrl: 'https://1rpc.io/eth',
    usdcContract: '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48',
    usdtContract: '0xdAC17F958D2ee523a2206206994597C13D831ec7',
    daiContract: '0x6B175474E89094C44Da98b954EedeAC495271d0F',
    wethContract: '0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2',
  );

  static const base = ChainConfig(
    chainId: 8453,
    name: 'Base',
    rpcUrl: 'https://mainnet.base.org',
    usdcContract: '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913',
    usdtContract: '0xfde4C96c8593536E31F229EA8f37b2ADa2699bb2',
    daiContract: '0x50c5725949A6F0c72E6C4a641F24049A917DB0Cb',
    wethContract: '0x4200000000000000000000000000000000000006',
  );

  static const arbitrum = ChainConfig(
    chainId: 42161,
    name: 'Arbitrum',
    rpcUrl: 'https://arb1.arbitrum.io/rpc',
    usdcContract: '0xaf88d065e77c8cC2239327C5EDb3A432268e5831',
    usdtContract: '0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9',
    daiContract: '0xDA10009cBd5D07dd0CeCc66161FC93D7c9000da1',
    wethContract: '0x82aF49447D8a07e3bd95BD0d56f35241523fBab1',
  );

  static const optimism = ChainConfig(
    chainId: 10,
    name: 'Optimism',
    rpcUrl: 'https://mainnet.optimism.io',
    usdcContract: '0x0b2C639c533813f4Aa9D7837CAf62653d097Ff85',
    usdtContract: '0x94b008aA00579c1307B0EF2c499aD98a8ce58e58',
    daiContract: '0xDA10009cBd5D07dd0CeCc66161FC93D7c9000da1',
    wethContract: '0x4200000000000000000000000000000000000006',
  );

  static const bsc = ChainConfig(
    chainId: 56,
    name: 'BNB Chain',
    rpcUrl: 'https://bsc-dataseed.binance.org',
    usdcContract: '0x8AC76a51cc950d9822D68b83fE1Ad97B32Cd580d',
    usdtContract: '0x55d398326f99059fF775485246999027B3197955',
    daiContract: '0x1AF3F329e8BE154074D8769D1FFa4eE058B1DBc3',
  );

  static const polygon = ChainConfig(
    chainId: 137,
    name: 'Polygon',
    rpcUrl: 'https://1rpc.io/matic',
    usdcContract: '0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359',
    usdtContract: '0xc2132D05D31c914a87C6611C10748AEb04B58e8F',
    daiContract: '0x8f3Cf7ad23Cd3CaDbD9735AFf958023239c6A063',
    wethContract: '0x7ceB23fD6bC0adD59E62ac25578270cFf1b9f619',
  );

  static const all = [ethereum, base, arbitrum, optimism, bsc, polygon];

  static ChainConfig byId(int chainId) {
    return all.firstWhere(
      (c) => c.chainId == chainId,
      orElse: () => base,
    );
  }

  static String chainName(int chainId) => byId(chainId).name;

  String tokenContract(String symbol) {
    switch (symbol.toUpperCase()) {
      case 'USDC':
        return usdcContract;
      case 'USDT':
        return usdtContract;
      case 'DAI':
        return daiContract;
      case 'WETH':
        return wethContract;
      default:
        return '';
    }
  }
}

class RpcException implements Exception {
  final int code;
  final String message;

  RpcException(this.code, this.message);

  @override
  String toString() => 'RpcException($code): $message';
}

abstract class ChainService {
  Future<BigInt> getEthBalance(String address);
  Future<BigInt> getTokenBalance(String address, String tokenContract);
  Future<String> sendRawTransaction(String signedTxHex);
  Future<int> getTransactionCount(String address);
  Future<BigInt> getGasPrice();
  Future<BigInt> estimateGas(Map<String, dynamic> txParams);
  Future<BigInt?> getBaseFee();
  Future<BigInt> getMaxPriorityFeePerGas();
  Future<Map<String, dynamic>?> getTransactionReceipt(String txHash);
  String tokenContract(String symbol);
  ChainConfig get currentConfig;
}

class JsonRpcChainService implements ChainService {
  final Dio _dio;
  String _rpcUrl;
  ChainConfig _config;
  int _requestId = 0;

  JsonRpcChainService({Dio? dio, ChainConfig? config})
      : _dio = dio ?? Dio(),
        _config = config ?? ChainConfig.base,
        _rpcUrl = (config ?? ChainConfig.base).rpcUrl {
    _dio.options.connectTimeout = const Duration(seconds: 15);
    _dio.options.receiveTimeout = const Duration(seconds: 15);
  }

  void switchChain(ChainConfig config) {
    _config = config;
    _rpcUrl = config.rpcUrl;
  }

  @override
  ChainConfig get currentConfig => _config;

  @override
  String tokenContract(String symbol) => _config.tokenContract(symbol);

  Future<dynamic> _call(String method, List<dynamic> params) async {
    _requestId++;
    final body = {
      'jsonrpc': '2.0',
      'method': method,
      'params': params,
      'id': _requestId,
    };

    final response = await _dio.post<Map<String, dynamic>>(
      _rpcUrl,
      data: body,
    );

    final data = response.data!;
    if (data.containsKey('error')) {
      final err = data['error'] as Map<String, dynamic>;
      throw RpcException(
        err['code'] as int,
        err['message'] as String,
      );
    }
    return data['result'];
  }

  @override
  Future<BigInt> getEthBalance(String address) async {
    final result = await _call('eth_getBalance', [address, 'latest']);
    return BigInt.parse(result as String);
  }

  @override
  Future<BigInt> getTokenBalance(
      String address, String tokenContract) async {
    // balanceOf(address) selector = 0x70a08231
    final addrStripped = address.toLowerCase().replaceFirst('0x', '');
    final calldata = '0x70a08231${addrStripped.padLeft(64, '0')}';

    final result = await _call('eth_call', [
      {'to': tokenContract, 'data': calldata},
      'latest',
    ]);
    final hexStr = result as String;
    if (hexStr == '0x' || hexStr.isEmpty) return BigInt.zero;
    return BigInt.parse(hexStr);
  }

  @override
  Future<String> sendRawTransaction(String signedTxHex) async {
    final result = await _call('eth_sendRawTransaction', [signedTxHex]);
    return result as String;
  }

  @override
  Future<int> getTransactionCount(String address) async {
    final result =
        await _call('eth_getTransactionCount', [address, 'latest']);
    return int.parse(result as String);
  }

  @override
  Future<BigInt> getGasPrice() async {
    final result = await _call('eth_gasPrice', []);
    return BigInt.parse(result as String);
  }

  @override
  Future<BigInt> estimateGas(Map<String, dynamic> txParams) async {
    final result = await _call('eth_estimateGas', [txParams]);
    return BigInt.parse(result as String);
  }

  @override
  Future<BigInt?> getBaseFee() async {
    final result = await _call('eth_getBlockByNumber', ['latest', false]);
    if (result == null) return null;
    final block = result as Map<String, dynamic>;
    final baseFeeHex = block['baseFeePerGas'] as String?;
    if (baseFeeHex == null) return null;
    return BigInt.parse(baseFeeHex);
  }

  @override
  Future<BigInt> getMaxPriorityFeePerGas() async {
    try {
      final result = await _call('eth_maxPriorityFeePerGas', []);
      final suggested = BigInt.parse(result as String);
      final floor = _minPriorityFee(_config.chainId);
      return suggested > floor ? suggested : floor;
    } catch (_) {
      // Fallback: gasPrice - baseFee, or chain minimum
      final gasPrice = await getGasPrice();
      final baseFee = await getBaseFee();
      final floor = _minPriorityFee(_config.chainId);
      if (baseFee != null && gasPrice > baseFee) {
        final derived = gasPrice - baseFee;
        return derived > floor ? derived : floor;
      }
      return floor;
    }
  }

  static BigInt _minPriorityFee(int chainId) {
    switch (chainId) {
      case 137: return BigInt.from(30000000000); // Polygon: 30 gwei
      case 56:  return BigInt.from(3000000000);  // BSC: 3 gwei
      default:  return BigInt.from(1000000000);  // Others: 1 gwei
    }
  }

  @override
  Future<Map<String, dynamic>?> getTransactionReceipt(String txHash) async {
    final result = await _call('eth_getTransactionReceipt', [txHash]);
    if (result == null) return null;
    return result as Map<String, dynamic>;
  }
}
