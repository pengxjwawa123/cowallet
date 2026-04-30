import '../network/dio_client.dart';
import '../network/result.dart';

/// MPC安全多方计算API - 匹配后端实际接口
class MpcApi {
  /// 创建MPC会话
  /// [sessionType] 会话类型，如 "keygen", "sign"
  /// [parties] 参与方ID列表
  /// [threshold] 门限值，默认为2
  static Future<Result<Map<String, dynamic>>> createSession({
    required String sessionType,
    required List<int> parties,
    int threshold = 2,
  }) async {
    return await DioClient.post(
      "/mpc/session",
      data: {
        "session_type": sessionType,
        "parties": parties,
        "threshold": threshold,
      },
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

  /// 接收MPC消息
  /// [sessionId] 会话ID
  /// 返回按轮次排序的消息列表
  static Future<Result<List<dynamic>>> receiveMessages(String sessionId) async {
    return await DioClient.get("/mpc/session/$sessionId/msg");
  }
}
