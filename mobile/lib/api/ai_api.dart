import '../network/dio_client.dart';
import '../network/result.dart';

/// AI助手API - 匹配后端实际接口
class AiApi {
  /// 发送聊天消息
  /// [message] 用户输入的消息内容
  /// [history] 历史对话消息列表，用于上下文理解
  /// 返回AI回复消息和可能的工具调用
  static Future<Result<Map<String, dynamic>>> chat({
    required String message,
    List<Map<String, dynamic>>? history,
  }) async {
    return await DioClient.post(
      "/ai/chat",
      data: {
        "message": message,
        "history": history ?? [],
      },
    );
  }

  /// 意图分类
  /// 将用户输入的自然语言分类为钱包操作意图
  /// 如：转账、查询余额、查看历史、创建钱包等
  static Future<Result<Map<String, dynamic>>> classifyIntent(String message) async {
    return await DioClient.post(
      "/ai/classify",
      data: {"message": message},
    );
  }
}

/// 聊天消息模型
class ChatMessage {
  final String role; // "user" 或 "assistant"
  final String content;

  ChatMessage({
    required this.role,
    required this.content,
  });

  Map<String, dynamic> toJson() => {
    "role": role,
    "content": content,
  };

  factory ChatMessage.fromJson(Map<String, dynamic> json) => ChatMessage(
    role: json["role"] as String,
    content: json["content"] as String,
  );
}
