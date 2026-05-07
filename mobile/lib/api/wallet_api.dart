/// 钱包API - 多钱包管理接口
/// 支持创建、查询、管理多个MPC钱包及其关联链

import '../network/dio_client.dart';
import '../network/result.dart';
import 'tx_api.dart';
import 'price_api.dart';

export 'tx_api.dart' show TxApi;
export 'price_api.dart' show PriceApi;

/// 钱包API - 多钱包CRUD与链管理
class WalletApi {
  /// 获取当前用户的所有钱包列表
  /// GET /api/v1/wallets
  static Future<Result<List<dynamic>>> listWallets() async {
    return await DioClient.get('/wallets');
  }

  /// 创建新钱包
  /// POST /api/v1/wallets
  /// [name] 钱包名称
  /// [publicKeyHex] MPC生成的公钥（hex编码）
  /// [chainIds] 支持的链ID列表（如 [1, 8453, 42161]）
  static Future<Result<Map<String, dynamic>>> createWallet({
    required String name,
    required String publicKeyHex,
    required List<int> chainIds,
  }) async {
    return await DioClient.post(
      '/wallets',
      data: {
        'name': name,
        'public_key_hex': publicKeyHex,
        'chain_ids': chainIds,
      },
    );
  }

  /// 获取指定钱包详情
  /// GET /api/v1/wallets/{id}
  /// [id] 钱包ID
  static Future<Result<Map<String, dynamic>>> getWallet(String id) async {
    return await DioClient.get('/wallets/$id');
  }

  /// 为钱包添加新链支持
  /// POST /api/v1/wallets/{walletId}/chains
  /// [walletId] 钱包ID
  /// [chainId] 要添加的链ID
  static Future<Result<Map<String, dynamic>>> addChain({
    required String walletId,
    required int chainId,
  }) async {
    return await DioClient.post(
      '/wallets/$walletId/chains',
      data: {
        'chain_id': chainId,
      },
    );
  }

  /// 移除钱包的链支持
  /// DELETE /api/v1/wallets/{walletId}/chains/{chainId}
  /// [walletId] 钱包ID
  /// [chainId] 要移除的链ID
  static Future<Result<Map<String, dynamic>>> removeChain({
    required String walletId,
    required int chainId,
  }) async {
    return await DioClient.delete('/wallets/$walletId/chains/$chainId');
  }

  /// 获取钱包总价值（通过交易历史和价格计算）
  /// 这是一个示例方法，实际余额应该在链上查询
  static Future<double> getEstimatedValue(String address) async {
    // 1. 获取交易历史
    var txResult = await TxApi.getHistory();
    if (!txResult.isSuccess) return 0;

    // 2. 获取ETH当前价格
    double? ethPrice = await PriceApi.getEthPrice();

    // 3. 计算估算价值（简化示例）
    // 实际项目中应该用web3dart在本地查询链上余额
    return ethPrice ?? 0;
  }
}
