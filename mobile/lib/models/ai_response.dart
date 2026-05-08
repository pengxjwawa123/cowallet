class AiToolCall {
  final String id;
  final String name;
  final Map<String, dynamic> parameters;

  AiToolCall({required this.id, required this.name, required this.parameters});

  factory AiToolCall.fromJson(Map<String, dynamic> json) => AiToolCall(
        id: json['id'] as String? ?? '',
        name: json['name'] as String? ?? '',
        parameters: json['parameters'] as Map<String, dynamic>? ?? {},
      );
}

class AiToolResult {
  final String toolId;
  final String toolName;
  final bool success;
  final Map<String, dynamic> result;
  final String? error;

  AiToolResult({
    required this.toolId,
    required this.toolName,
    required this.success,
    required this.result,
    this.error,
  });

  factory AiToolResult.fromJson(Map<String, dynamic> json) => AiToolResult(
        toolId: json['tool_id'] as String? ?? '',
        toolName: json['tool_name'] as String? ?? '',
        success: json['success'] as bool? ?? false,
        result: json['result'] as Map<String, dynamic>? ?? {},
        error: json['error'] as String?,
      );
}

class AiChatResponse {
  final String message;
  final List<AiToolCall> toolCalls;
  final List<AiToolResult> toolResults;
  final List<String> needsConfirmation;

  AiChatResponse({
    required this.message,
    required this.toolCalls,
    required this.toolResults,
    required this.needsConfirmation,
  });

  factory AiChatResponse.fromJson(Map<String, dynamic> json) {
    final toolCallsList = (json['tool_calls'] as List<dynamic>?)
            ?.map((e) => AiToolCall.fromJson(e as Map<String, dynamic>))
            .toList() ??
        [];
    final toolResultsList = (json['tool_results'] as List<dynamic>?)
            ?.map((e) => AiToolResult.fromJson(e as Map<String, dynamic>))
            .toList() ??
        [];
    final confirmList = (json['needs_confirmation'] as List<dynamic>?)
            ?.map((e) => e as String)
            .toList() ??
        [];

    return AiChatResponse(
      message: json['message'] as String? ?? '',
      toolCalls: toolCallsList,
      toolResults: toolResultsList,
      needsConfirmation: confirmList,
    );
  }

  bool get hasToolResults => toolResults.isNotEmpty;
  bool get hasWriteTools => needsConfirmation.isNotEmpty;

  AiToolResult? getResultForTool(String toolName) {
    try {
      return toolResults.firstWhere((r) => r.toolName == toolName);
    } catch (_) {
      return null;
    }
  }
}
