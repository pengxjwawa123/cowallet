import '../network/dio_client.dart';
import '../network/result.dart';
import '../models/ai_response.dart';

class AiApi {
  /// Send chat message and get AI response with tool execution results.
  static Future<Result<AiChatResponse>> chat({
    required String message,
    List<Map<String, dynamic>>? history,
    String? walletAddress,
  }) async {
    final result = await DioClient.post<Map<String, dynamic>>(
      "/ai/chat",
      data: {
        "message": message,
        "history": history ?? [],
        if (walletAddress != null) "wallet_address": walletAddress,
      },
    );

    if (result.isSuccess && result.data != null) {
      final response = AiChatResponse.fromJson(result.data!);
      return Result.success(response);
    }
    return Result.error(result.errorMessage ?? 'AI request failed', result.errorCode ?? -1);
  }

  /// Classify user intent (lightweight, no tool execution).
  static Future<Result<Map<String, dynamic>>> classifyIntent(String message) async {
    return await DioClient.post("/ai/classify", data: {"message": message});
  }
}
