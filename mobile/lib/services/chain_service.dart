import 'package:dio/dio.dart';

class ChainConfig {
  final int chainId;
  final String rpcUrl;
  final String usdcContract;

  const ChainConfig({
    required this.chainId,
    required this.rpcUrl,
    required this.usdcContract,
  });

  static const baseSepolia = ChainConfig(
    chainId: 84532,
    rpcUrl: 'https://sepolia.base.org',
    usdcContract: '0x036CbD53842c5426634e7929541eC2318f3dCF7e',
  );
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
  Future<Map<String, dynamic>?> getTransactionReceipt(String txHash);
}

class JsonRpcChainService implements ChainService {
  final Dio _dio;
  final String _rpcUrl;
  int _requestId = 0;

  JsonRpcChainService({Dio? dio, String? rpcUrl})
      : _dio = dio ?? Dio(),
        _rpcUrl = rpcUrl ?? ChainConfig.baseSepolia.rpcUrl {
    _dio.options.connectTimeout = const Duration(seconds: 15);
    _dio.options.receiveTimeout = const Duration(seconds: 15);
  }

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
  Future<Map<String, dynamic>?> getTransactionReceipt(String txHash) async {
    final result = await _call('eth_getTransactionReceipt', [txHash]);
    if (result == null) return null;
    return result as Map<String, dynamic>;
  }
}
