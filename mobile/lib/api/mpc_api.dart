import '../network/dio_client.dart';
import '../network/result.dart';

/// MPC安全多方计算API - 匹配后端实际接口
class MpcApi {
  /// 创建MPC会话
  /// [sessionType] 会话类型，如 "keygen", "sign"
  /// [parties] 参与方ID列表
  /// [threshold] 门限值，默认为2
  /// [walletId] 可选，关联的钱包ID（签名会话时指定）
  static Future<Result<Map<String, dynamic>>> createSession({
    required String sessionType,
    required List<int> parties,
    int threshold = 2,
    String? walletId,
  }) async {
    final Map<String, dynamic> body = {
      "session_type": sessionType,
      "parties": parties,
      "threshold": threshold,
    };
    if (walletId != null) {
      body["wallet_id"] = walletId;
    }
    return await DioClient.post(
      "/mpc/session",
      data: body,
    );
  }

  /// 获取会话信息
  /// [sessionId] 会话ID
  static Future<Result<Map<String, dynamic>>> getSession(String sessionId) async {
    return await DioClient.get("/mpc/session/$sessionId");
  }

  /// 终止/取消会话
  /// [sessionId] 会话ID
  static Future<Result<bool>> abortSession(String sessionId) async {
    Result result = await DioClient.delete("/mpc/session/$sessionId");
    return Result.success(result.isSuccess);
  }

  /// 发送MPC消息
  /// [sessionId] 会话ID
  /// [fromParty] 发送方编号
  /// [toParty] 接收方编号
  /// [round] 当前轮次
  /// [payload] 加密消息payload
  static Future<Result<bool>> sendMessage({
    required String sessionId,
    required int fromParty,
    required int toParty,
    required int round,
    required List<int> payload,
  }) async {
    Result result = await DioClient.post(
      "/mpc/session/$sessionId/msg",
      data: {
        "from_party": fromParty,
        "to_party": toParty,
        "round": round,
        "payload": payload,
      },
    );
    return Result.success(result.isSuccess);
  }

  /// 接收MPC消息（按party过滤，支持增量轮询）
  /// [sessionId] 会话ID
  /// [party] 接收方party编号（只拉取发给该party的消息）
  /// [afterId] 只返回id大于此值的消息（增量轮询）
  static Future<Result<List<dynamic>>> receiveMessages(
    String sessionId, {
    required int party,
    int? afterId,
  }) async {
    String path = "/mpc/session/$sessionId/msg?party=$party";
    if (afterId != null) {
      path += "&after_id=$afterId";
    }
    return await DioClient.get(path);
  }

  /// 获取预签名状态
  /// GET /api/v1/mpc/presign/status?wallet_id={id}
  /// [walletId] 钱包ID，查询该钱包可用的预签名数量和状态
  static Future<Result<Map<String, dynamic>>> getPresignStatus(
    String walletId,
  ) async {
    return await DioClient.get(
      "/mpc/presign/status",
      params: {"wallet_id": walletId},
    );
  }

  /// 批量生成预签名
  /// POST /api/v1/mpc/presign/generate
  /// [walletId] 钱包ID
  /// [count] 要生成的预签名数量
  static Future<Result<Map<String, dynamic>>> generatePresignatures({
    required String walletId,
    required int count,
  }) async {
    return await DioClient.post(
      "/mpc/presign/generate",
      data: {
        "wallet_id": walletId,
        "count": count,
      },
    );
  }
}
