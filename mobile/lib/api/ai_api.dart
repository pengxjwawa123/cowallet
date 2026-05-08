import 'dart:async';
import 'dart:convert';
import '../network/dio_client.dart';
import '../network/result.dart';

/// SSE event from AI chat stream
class AiStreamEvent {
  final String event;
  final Map<String, dynamic> data;

  AiStreamEvent({required this.event, required this.data});
}

/// Session info from backend
class ChatSessionInfo {
  final String id;
  final String? title;
  final String createdAt;
  final String updatedAt;

  ChatSessionInfo({
    required this.id,
    this.title,
    required this.createdAt,
    required this.updatedAt,
  });

  factory ChatSessionInfo.fromJson(Map<String, dynamic> json) => ChatSessionInfo(
        id: json['id'] as String? ?? '',
        title: json['title'] as String?,
        createdAt: json['created_at'] as String? ?? '',
        updatedAt: json['updated_at'] as String? ?? '',
      );
}

class AiApi {
  /// Stream chat — returns a Stream of SSE events.
  /// Events: session, token, tool_call, tool_result, done, error
  static Stream<AiStreamEvent> chatStream({
    required String message,
    String? sessionId,
    String? userId,
  }) async* {
    final response = await DioClient.postStream(
      "/ai/chat",
      data: {
        "message": message,
        if (sessionId != null) "session_id": sessionId,
        if (userId != null) "user_id": userId,
      },
    );

    if (response == null) {
      yield AiStreamEvent(event: "error", data: {"message": "请求失败"});
      return;
    }

    String buffer = '';

    await for (final chunk in response) {
      buffer += chunk;

      // Parse SSE events (separated by \n\n)
      while (buffer.contains('\n\n')) {
        final idx = buffer.indexOf('\n\n');
        final block = buffer.substring(0, idx);
        buffer = buffer.substring(idx + 2);

        String? eventName;
        String? eventData;

        for (final line in block.split('\n')) {
          if (line.startsWith('event: ')) {
            eventName = line.substring(7);
          } else if (line.startsWith('data: ')) {
            eventData = line.substring(6);
          }
        }

        if (eventName != null && eventData != null) {
          try {
            final data = jsonDecode(eventData) as Map<String, dynamic>;
            yield AiStreamEvent(event: eventName, data: data);
          } catch (_) {
            yield AiStreamEvent(event: eventName, data: {"raw": eventData});
          }
        }
      }
    }
  }

  /// Create a new chat session
  static Future<Result<ChatSessionInfo>> createSession({
    required String userId,
    String? title,
  }) async {
    final result = await DioClient.post<Map<String, dynamic>>(
      "/ai/sessions",
      data: {
        "user_id": userId,
        if (title != null) "title": title,
      },
    );

    if (result.isSuccess && result.data != null) {
      return Result.success(ChatSessionInfo.fromJson(result.data!));
    }
    return Result.error(result.errorMessage ?? 'Failed to create session', result.errorCode ?? -1);
  }

  /// List chat sessions for a user
  static Future<Result<List<ChatSessionInfo>>> listSessions({
    required String userId,
  }) async {
    final result = await DioClient.get<List<dynamic>>(
      "/ai/sessions",
      queryParameters: {"user_id": userId},
    );

    if (result.isSuccess && result.data != null) {
      final sessions = result.data!
          .map((e) => ChatSessionInfo.fromJson(e as Map<String, dynamic>))
          .toList();
      return Result.success(sessions);
    }
    return Result.error(result.errorMessage ?? 'Failed to list sessions', result.errorCode ?? -1);
  }

  /// Get messages for a session
  static Future<Result<List<Map<String, dynamic>>>> getSessionMessages({
    required String sessionId,
  }) async {
    final result = await DioClient.get<List<dynamic>>(
      "/ai/sessions/$sessionId/messages",
    );

    if (result.isSuccess && result.data != null) {
      final messages = result.data!
          .map((e) => e as Map<String, dynamic>)
          .toList();
      return Result.success(messages);
    }
    return Result.error(result.errorMessage ?? 'Failed to load messages', result.errorCode ?? -1);
  }

  /// Delete a session
  static Future<Result<bool>> deleteSession({
    required String sessionId,
    required String userId,
  }) async {
    final result = await DioClient.delete<Map<String, dynamic>>(
      "/ai/sessions/$sessionId",
      queryParameters: {"user_id": userId},
    );

    if (result.isSuccess) {
      return Result.success(true);
    }
    return Result.error(result.errorMessage ?? 'Failed to delete session', result.errorCode ?? -1);
  }
}
