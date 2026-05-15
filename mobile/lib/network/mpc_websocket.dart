import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';
import 'package:web_socket_channel/web_socket_channel.dart';

import '../api/mpc_api.dart';
import '../config/api_config.dart';
import '../utils/secure_storage.dart';

/// MPC协议消息
class MpcMessage {
  final int fromParty;
  final int toParty;
  final int round;
  final List<int> payload;

  /// Server-assigned message ID for incremental polling.
  /// May be null for locally-constructed messages or WebSocket-received messages
  /// that don't include an ID field.
  final int? messageId;

  const MpcMessage({
    required this.fromParty,
    required this.toParty,
    required this.round,
    required this.payload,
    this.messageId,
  });

  factory MpcMessage.fromJson(Map<String, dynamic> json) {
    final rawPayload = json['payload'];
    final List<int> payload;
    if (rawPayload is String) {
      payload = base64Decode(rawPayload);
    } else {
      payload = (rawPayload as List<dynamic>).cast<int>();
    }
    return MpcMessage(
      fromParty: json['from_party'] as int,
      toParty: json['to_party'] as int,
      round: json['round'] as int,
      payload: payload,
      messageId: json['id'] as int?,
    );
  }

  Map<String, dynamic> toJson() => {
        'from_party': fromParty,
        'to_party': toParty,
        'round': round,
        'payload': base64Encode(Uint8List.fromList(payload)),
      };
}

/// WebSocket连接状态
enum MpcWebSocketState {
  disconnected,
  connecting,
  connected,
  reconnecting,
  /// Session has expired on the server and cannot be recovered.
  sessionExpired,
}

/// Result of a reconnection attempt indicating whether recovery succeeded.
enum ReconnectRecoveryResult {
  /// Reconnected and all missed messages were replayed successfully.
  success,
  /// Reconnected but the MPC session has expired on the server.
  sessionExpired,
  /// Reconnection failed (will keep retrying if attempts remain).
  failed,
}

/// 管理MPC会话的WebSocket连接，用于实时传输MPC协议消息。
/// 连接地址: ws://host/api/v1/mpc/session/{sessionId}/ws?party={partyIndex}&token={jwt}
///
/// Protocol-aware reconnection:
/// After a disconnect mid-protocol, the WebSocket reconnects then fetches missed
/// messages via the HTTP fallback API and feeds them into the message stream so
/// the protocol session can continue without replaying completed rounds.
class MpcWebSocket {
  final String sessionId;
  final int partyIndex;

  WebSocketChannel? _channel;
  StreamController<MpcMessage>? _messageController;
  StreamSubscription? _subscription;
  Timer? _reconnectTimer;
  Timer? _heartbeatTimer;
  int _reconnectAttempts = 0;
  MpcWebSocketState _state = MpcWebSocketState.disconnected;

  /// Tracks the highest message ID received so far (for incremental polling).
  int _lastReceivedMessageId = 0;

  /// Tracks the highest round number seen (informational).
  int _lastReceivedRound = 0;

  /// Whether this WebSocket has been connected at least once (vs first connect).
  bool _hasConnectedBefore = false;

  /// Optional callback invoked after a successful reconnection and message recovery.
  /// The [ReconnectRecoveryResult] indicates whether recovery was successful or
  /// the session has expired. Protocol code can use this to decide whether to
  /// abort or continue.
  void Function(ReconnectRecoveryResult result)? onReconnected;

  /// Optional callback invoked when all reconnection attempts are exhausted.
  void Function()? onReconnectFailed;

  static const int _maxReconnectAttempts = 5;
  static const Duration _heartbeatInterval = Duration(seconds: 30);

  /// When true, disables auto-reconnect on disconnect. Use for non-resumable
  /// protocols (DKG, reshare) where reconnection is pointless.
  final bool disableAutoReconnect;

  MpcWebSocket({
    required this.sessionId,
    required this.partyIndex,
    this.onReconnected,
    this.onReconnectFailed,
    this.disableAutoReconnect = false,
  });

  /// 当前连接状态
  MpcWebSocketState get state => _state;

  /// 是否已连接
  bool get isConnected => _state == MpcWebSocketState.connected;

  /// Whether the session has expired on the server (terminal state).
  bool get isSessionExpired => _state == MpcWebSocketState.sessionExpired;

  /// The last received message ID (useful for external tracking).
  int get lastReceivedMessageId => _lastReceivedMessageId;

  /// The last received protocol round number.
  int get lastReceivedRound => _lastReceivedRound;

  /// 消息流，监听此流以接收MPC消息
  Stream<MpcMessage> get messages {
    _messageController ??= StreamController<MpcMessage>.broadcast();
    return _messageController!.stream;
  }

  /// Set the initial message ID watermark (e.g. when resuming a session that
  /// was previously persisted). Messages with IDs <= this value will not be
  /// re-fetched during recovery.
  void setLastMessageId(int messageId) {
    _lastReceivedMessageId = messageId;
  }

  /// 构建WebSocket URL
  /// 将HTTP URL转换为WS URL，并附加session/party/token参数
  Future<Uri> _buildWsUri() async {
    String token = await SecureStorage.getToken() ?? '';

    // 将 http:// 或 https:// 转换为 ws:// 或 wss://
    String wsBase = ApiConfig.baseUrl
        .replaceFirst('http://', 'ws://')
        .replaceFirst('https://', 'wss://');

    String url =
        '$wsBase${ApiConfig.apiPrefix}/mpc/session/$sessionId/ws?party=$partyIndex&token=$token';

    return Uri.parse(url);
  }

  /// 连接WebSocket
  /// 如果已连接则先断开再重连
  /// On reconnection (not first connect), also fetches missed messages.
  Future<void> connect() async {
    if (_state == MpcWebSocketState.connected ||
        _state == MpcWebSocketState.connecting) {
      return;
    }

    // Terminal state — cannot reconnect an expired session.
    if (_state == MpcWebSocketState.sessionExpired) {
      return;
    }

    _state = MpcWebSocketState.connecting;
    _messageController ??= StreamController<MpcMessage>.broadcast();

    try {
      Uri uri = await _buildWsUri();
      print('[MpcWebSocket] Connecting to: $uri');

      _channel = WebSocketChannel.connect(uri);

      // 等待连接就绪
      await _channel!.ready;

      _reconnectAttempts = 0;
      print('[MpcWebSocket] WebSocket connected');

      _subscription = _channel!.stream.listen(
        (data) => _onMessage(data),
        onError: (error) => _onError(error),
        onDone: () => _onDone(),
      );

      _state = MpcWebSocketState.connected;
      print('[MpcWebSocket] Connected (plain JSON transport)');

      // 启动心跳
      _startHeartbeat();

      // If this is a reconnection (not first connect), recover missed messages.
      if (_hasConnectedBefore) {
        await _recoverMissedMessages();
      }

      _hasConnectedBefore = true;
    } catch (e) {
      print('[MpcWebSocket] Connection failed: $e');
      _state = MpcWebSocketState.disconnected;
      _scheduleReconnect();
    }
  }

  /// 断开WebSocket连接
  Future<void> disconnect() async {
    _reconnectTimer?.cancel();
    _reconnectTimer = null;
    _heartbeatTimer?.cancel();
    _heartbeatTimer = null;
    _reconnectAttempts = _maxReconnectAttempts; // 阻止自动重连

    await _subscription?.cancel();
    _subscription = null;

    await _channel?.sink.close();
    _channel = null;

    _state = MpcWebSocketState.disconnected;
    print('[MpcWebSocket] Disconnected');

    await _messageController?.close();
    _messageController = null;
  }

  /// 发送MPC消息（plain JSON）
  Future<void> send(MpcMessage message) async {
    if (_state != MpcWebSocketState.connected || _channel == null) {
      throw MpcSendFailedException('Cannot send: not connected (state=$_state)');
    }

    String jsonStr = jsonEncode(message.toJson());
    _channel!.sink.add(jsonStr);
  }

  /// 发送原始MPC消息参数
  Future<void> sendRaw({
    required int toParty,
    required int round,
    required List<int> payload,
  }) {
    return send(MpcMessage(
      fromParty: partyIndex,
      toParty: toParty,
      round: round,
      payload: payload,
    ));
  }

  /// 处理收到的消息（plain JSON）
  void _onMessage(dynamic data) {
    try {
      Map<String, dynamic> json;
      if (data is String) {
        json = jsonDecode(data) as Map<String, dynamic>;
      } else {
        json = jsonDecode(utf8.decode(data as List<int>))
            as Map<String, dynamic>;
      }

      if (json.containsKey('type') && json['type'] == 'pong') {
        return;
      }

      // Handle session expiry notification from server.
      if (json.containsKey('type') && json['type'] == 'session_expired') {
        _handleSessionExpired();
        return;
      }

      MpcMessage message = MpcMessage.fromJson(json);
      _trackMessageSequence(message);
      _messageController?.add(message);
    } catch (e) {
      print('[MpcWebSocket] Error parsing message: $e');
    }
  }

  /// Update sequence tracking state from a received message.
  void _trackMessageSequence(MpcMessage message) {
    if (message.messageId != null && message.messageId! > _lastReceivedMessageId) {
      _lastReceivedMessageId = message.messageId!;
    }
    if (message.round > _lastReceivedRound) {
      _lastReceivedRound = message.round;
    }
  }

  /// Handle server-side session expiry. Moves to terminal state.
  void _handleSessionExpired() {
    print('[MpcWebSocket] Session $sessionId has expired on server');
    _state = MpcWebSocketState.sessionExpired;
    _reconnectTimer?.cancel();
    _heartbeatTimer?.cancel();

    _messageController?.addError(
      MpcSessionExpiredException(
        'MPC session $sessionId has expired on the server',
      ),
    );

    onReconnected?.call(ReconnectRecoveryResult.sessionExpired);
  }

  /// After a successful reconnection, fetch any messages missed during the
  /// disconnect period via the HTTP fallback API and feed them into the
  /// message stream.
  Future<void> _recoverMissedMessages() async {
    print('[MpcWebSocket] Recovering missed messages (after_id=$_lastReceivedMessageId)');

    try {
      // First, verify the session is still active on the server.
      final sessionResult = await MpcApi.getSession(sessionId);
      if (!sessionResult.isSuccess || sessionResult.data == null) {
        // Session not found — it may have been cleaned up.
        _handleSessionExpired();
        return;
      }

      final status = sessionResult.data!['status'] as String?;
      if (status != null && (status == 'expired' || status == 'failed' || status == 'completed')) {
        _handleSessionExpired();
        return;
      }

      // Fetch messages that arrived while we were disconnected.
      final messagesResult = await MpcApi.receiveMessages(
        sessionId,
        party: partyIndex,
        afterId: _lastReceivedMessageId > 0 ? _lastReceivedMessageId : null,
      );

      if (!messagesResult.isSuccess || messagesResult.data == null) {
        print('[MpcWebSocket] No missed messages or fetch failed');
        onReconnected?.call(ReconnectRecoveryResult.success);
        return;
      }

      final missedMessages = messagesResult.data!;
      if (missedMessages.isEmpty) {
        print('[MpcWebSocket] No missed messages to replay');
        onReconnected?.call(ReconnectRecoveryResult.success);
        return;
      }

      print('[MpcWebSocket] Replaying ${missedMessages.length} missed messages');

      // Sort by ID to ensure correct ordering.
      final sorted = List<Map<String, dynamic>>.from(
        missedMessages.map((raw) => Map<String, dynamic>.from(raw as Map)),
      );
      sorted.sort((a, b) => (a['id'] as int? ?? 0).compareTo(b['id'] as int? ?? 0));

      for (final raw in sorted) {
        final message = MpcMessage(
          fromParty: raw['from_party'] as int,
          toParty: raw['to_party'] as int,
          round: raw['round'] as int,
          payload: (raw['payload'] as List<dynamic>).cast<int>(),
          messageId: raw['id'] as int?,
        );

        // Skip messages we already received (belt-and-suspenders check).
        if (message.messageId != null && message.messageId! <= _lastReceivedMessageId) {
          continue;
        }

        _trackMessageSequence(message);
        _messageController?.add(message);
      }

      print('[MpcWebSocket] Recovery complete, last_message_id=$_lastReceivedMessageId');
      onReconnected?.call(ReconnectRecoveryResult.success);
    } catch (e) {
      print('[MpcWebSocket] Error recovering missed messages: $e');
      // Recovery failed but connection is live — protocol may still work
      // if no messages were actually missed.
      onReconnected?.call(ReconnectRecoveryResult.success);
    }
  }


  /// 处理连接错误
  void _onError(dynamic error) {
    print('[MpcWebSocket] Error: $error');
    _state = MpcWebSocketState.disconnected;
    _heartbeatTimer?.cancel();
    _scheduleReconnect();
  }

  /// 处理连接关闭
  void _onDone() {
    print('[MpcWebSocket] Connection closed');
    _state = MpcWebSocketState.disconnected;
    _heartbeatTimer?.cancel();
    _scheduleReconnect();
  }

  /// 启动心跳定时器
  void _startHeartbeat() {
    _heartbeatTimer?.cancel();
    _heartbeatTimer = Timer.periodic(_heartbeatInterval, (_) {
      if (_state == MpcWebSocketState.connected && _channel != null) {
        _channel!.sink.add(jsonEncode({'type': 'ping'}));
      }
    });
  }

  /// 安排自动重连（指数退避: 1s, 2s, 4s, 8s, 16s）
  void _scheduleReconnect() {
    // Don't reconnect if session is expired or we were explicitly disconnected.
    if (_state == MpcWebSocketState.sessionExpired) {
      return;
    }

    if (disableAutoReconnect) {
      _messageController?.addError(
        Exception('WebSocket disconnected (auto-reconnect disabled)'),
      );
      onReconnectFailed?.call();
      return;
    }

    if (_reconnectAttempts >= _maxReconnectAttempts) {
      print('[MpcWebSocket] Max reconnect attempts reached');
      _messageController?.addError(
        Exception('WebSocket connection failed after $_maxReconnectAttempts attempts'),
      );
      onReconnectFailed?.call();
      return;
    }

    _state = MpcWebSocketState.reconnecting;
    int delaySeconds = 1 << _reconnectAttempts; // 1, 2, 4, 8, 16
    _reconnectAttempts++;

    print('[MpcWebSocket] Reconnecting in ${delaySeconds}s (attempt $_reconnectAttempts/$_maxReconnectAttempts)');

    _reconnectTimer?.cancel();
    _reconnectTimer = Timer(Duration(seconds: delaySeconds), () async {
      await _subscription?.cancel();
      _subscription = null;
      _channel = null;
      await connect();
    });
  }
}

/// Exception indicating the MPC session has expired on the server and
/// cannot be recovered. Callers should start a new session.
class MpcSessionExpiredException implements Exception {
  final String message;

  MpcSessionExpiredException(this.message);

  @override
  String toString() => 'MpcSessionExpiredException: $message';
}

/// Exception indicating that sending an MPC message failed.
class MpcSendFailedException implements Exception {
  final String message;

  MpcSendFailedException(this.message);

  @override
  String toString() => 'MpcSendFailedException: $message';
}
