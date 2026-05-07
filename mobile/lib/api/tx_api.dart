import '../network/dio_client.dart';
import '../network/result.dart';

/// 交易API - 匹配后端实际接口
class TxApi {
  /// 提交已签名的交易
  /// [rawTx] 已签名的原始交易hex字符串
  /// [chainId] 链ID，默认84532 (Base Sepolia)
  /// [toAddr] 接收地址（可选，用于记录）
  /// [value] 转账金额（可选，用于记录）
  /// [token] 代币符号（可选，用于记录）
  /// 返回交易hash
  static Future<Result<Map<String, dynamic>>> submit({
    required String rawTx,
    int? chainId,
    String? toAddr,
    String? value,
    String? token,
  }) async {
    return await DioClient.post(
      "/tx/submit",
      data: {
        "raw_tx": rawTx,
        if (chainId != null) "chain_id": chainId,
        if (toAddr != null) "to_addr": toAddr,
        if (value != null) "value": value,
        if (token != null) "token": token,
      },
    );
  }

  /// 获取交易历史记录（新版本 - 按地址查询）
  /// [address] 钱包地址
  /// [chainId] 可选的链ID筛选
  /// [limit] 每页数量，默认50，最大100
  /// [offset] 偏移量，用于分页
  static Future<Result<Map<String, dynamic>>> getTransactionHistory(
    String address, {
    int? chainId,
    int limit = 50,
    int offset = 0,
  }) async {
    return await DioClient.get(
      "/tx/history",
      params: {
        "address": address,
        if (chainId != null) "chain_id": chainId,
        "limit": limit,
        "offset": offset,
      },
    );
  }

  /// 获取单个交易详情
  /// [txHash] 交易哈希
  static Future<Result<Map<String, dynamic>>> getTransaction(String txHash) async {
    return await DioClient.get("/tx/$txHash");
  }

  /// 获取交易历史记录（旧版本 - 按用户查询，保留向后兼容）
  /// [limit] 每页数量，默认20，最大100
  /// [offset] 偏移量，用于分页
  @Deprecated('Use getTransactionHistory with address parameter instead')
  static Future<Result<List<dynamic>>> getHistory({
    int limit = 20,
    int offset = 0,
  }) async {
    Result<Map<String, dynamic>> result = await DioClient.get(
      "/tx/history",
      params: {
        "limit": limit,
        "offset": offset,
      },
    );

    if (result.isSuccess) {
      List<dynamic> transactions = result.data?["transactions"] ?? [];
      return Result.success(transactions);
    }
    return Result.error(result.errorMessage ?? "获取交易历史失败", -1);
  }

  /// 模拟交易（eth_call）
  /// [to] 目标合约地址
  /// [value] 发送的ETH金额（可选）
  /// [data] 合约调用数据（可选）
  /// [from] 调用者地址（可选）
  static Future<Result<Map<String, dynamic>>> simulate({
    required String to,
    String? value,
    String? data,
    String? from,
  }) async {
    return await DioClient.post(
      "/tx/simulate",
      data: {
        "to": to,
        if (value != null) "value": value,
        if (data != null) "data": data,
        if (from != null) "from": from,
      },
    );
  }
}
