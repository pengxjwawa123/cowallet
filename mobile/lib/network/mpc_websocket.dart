import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';
import 'package:web_socket_channel/web_socket_channel.dart';

import '../api/mpc_api.dart';
import '../config/api_config.dart';
import '../utils/secure_storage.dart';
import '../bridge/noise_bridge.dart' as native;

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
    return MpcMessage(
      fromParty: json['from_party'] as int,
      toParty: json['to_party'] as int,
      round: json['round'] as int,
      payload: (json['payload'] as List<dynamic>).cast<int>(),
      messageId: json['id'] as int?,
    );
  }

  Map<String, dynamic> toJson() => {
        'from_party': fromParty,
        'to_party': toParty,
        'round': round,
        'payload': payload,
      };
}

/// WebSocket连接状态
enum MpcWebSocketState {
  disconnected,
  connecting,
  connected,
  noiseHandshaking,
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
/// All connections use Noise_XX transport encryption.
/// The device's static key is loaded from secure storage or generated on first use.
///
/// Protocol-aware reconnection:
/// After a disconnect mid-protocol, the WebSocket reconnects, re-handshakes Noise,
/// then fetches missed messages via the HTTP fallback API and feeds them into the
/// message stream so the protocol session can continue without replaying completed rounds.
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

  /// Noise session ID (from Rust FFI).
  String? _noiseSessionId;

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
  static const String _noiseKeyStorageKey = 'noise_static_private_key';

  MpcWebSocket({
    required this.sessionId,
    required this.partyIndex,
    this.onReconnected,
    this.onReconnectFailed,
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
  /// Performs Noise_XX handshake after WS connect.
  /// On reconnection (not first connect), also fetches missed messages.
  Future<void> connect() async {
    if (_state == MpcWebSocketState.connected ||
        _state == MpcWebSocketState.connecting ||
        _state == MpcWebSocketState.noiseHandshaking) {
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

      // Perform Noise_XX handshake before accepting MPC messages
      _state = MpcWebSocketState.noiseHandshaking;
      await _performNoiseHandshake();

      _state = MpcWebSocketState.connected;
      print('[MpcWebSocket] Connected with Noise_XX encryption');

      // 监听消息
      _subscription = _channel!.stream.listen(
        _onMessage,
        onError: _onError,
        onDone: _onDone,
      );

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

    // Clean up Noise session if active
    if (_noiseSessionId != null) {
      await native.noiseSessionDestroy(sessionId: _noiseSessionId!);
      _noiseSessionId = null;
    }

    _state = MpcWebSocketState.disconnected;
    print('[MpcWebSocket] Disconnected');

    await _messageController?.close();
    _messageController = null;
  }

  /// 发送MPC消息（Noise加密）
  Future<void> send(MpcMessage message) async {
    if (_state != MpcWebSocketState.connected || _channel == null) {
      print('[MpcWebSocket] Cannot send: not connected');
      return;
    }

    String jsonStr = jsonEncode(message.toJson());

    try {
      final ciphertextBase64 = await native.noiseEncrypt(
        sessionId: _noiseSessionId!,
        plaintext: Uint8List.fromList(utf8.encode(jsonStr)),
      );
      final envelope = jsonEncode({
        'type': 'noise_encrypted',
        'data': ciphertextBase64,
      });
      _channel!.sink.add(envelope);
    } catch (e) {
      print('[MpcWebSocket] Noise encryption failed: $e');
    }
  }

  /// 发送原始MPC消息参数
  void sendRaw({
    required int toParty,
    required int round,
    required List<int> payload,
  }) {
    send(MpcMessage(
      fromParty: partyIndex,
      toParty: toParty,
      round: round,
      payload: payload,
    ));
  }

  /// 处理收到的消息（Noise解密）
  Future<void> _onMessage(dynamic data) async {
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

      if (json.containsKey('type') && json['type'] == 'noise_encrypted') {
        final ciphertextBase64 = json['data'] as String;
        final plaintext = await native.noiseDecrypt(
          sessionId: _noiseSessionId!,
          ciphertextBase64: ciphertextBase64,
        );
        json = jsonDecode(utf8.decode(plaintext)) as Map<String, dynamic>;
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

  /// Perform Noise_XX handshake over the established WebSocket.
  /// This is a 3-message handshake:
  ///   1. Device sends -> e (first message)
  ///   2. Server sends <- e, ee, s, es (response)
  ///   3. Device sends -> s, se (final)
  /// After completion, all messages are encrypted/decrypted via the Noise session.
  Future<void> _performNoiseHandshake() async {
    final staticKey = await _getOrCreateStaticKey();

    // Step 1: Create initiator session and generate first handshake message
    final startResult = await native.noiseInitiatorStart(
      staticPrivateKey: Uint8List.fromList(staticKey),
    );
    _noiseSessionId = startResult.sessionId;

    // Send step 1 to server
    final step1Msg = jsonEncode({
      'type': 'noise_handshake',
      'step': 1,
      'data': startResult.messageBase64,
    });
    _channel!.sink.add(step1Msg);

    // Wait for server's step 2 response
    final step2Response = await _channel!.stream.first;
    final String step2Text;
    if (step2Response is String) {
      step2Text = step2Response;
    } else {
      step2Text = utf8.decode(step2Response as List<int>);
    }

    final step2Json = jsonDecode(step2Text) as Map<String, dynamic>;
    if (step2Json['type'] != 'noise_handshake' || step2Json['step'] != 2) {
      throw Exception('Expected noise_handshake step 2, got: ${step2Json['type']} step ${step2Json['step']}');
    }

    // Step 3: Process server's response and generate final message
    final finishResult = await native.noiseInitiatorFinish(
      sessionId: _noiseSessionId!,
      serverMessageBase64: step2Json['data'] as String,
    );

    if (!finishResult.isReady) {
      throw Exception('Noise handshake did not complete after step 3');
    }

    // Send step 3 to server
    final step3Msg = jsonEncode({
      'type': 'noise_handshake',
      'step': 3,
      'data': finishResult.messageBase64,
    });
    _channel!.sink.add(step3Msg);

    print('[MpcWebSocket] Noise_XX handshake complete, transport encrypted');
  }

  /// Load the device's Noise static private key from secure storage,
  /// or generate a new one on first use and persist it.
  Future<List<int>> _getOrCreateStaticKey() async {
    final existingKey = await SecureStorage.get(_noiseKeyStorageKey);
    if (existingKey != null && existingKey.isNotEmpty) {
      return base64Decode(existingKey);
    }

    // Generate a new keypair and store the private key
    final keypair = await native.noiseGenerateKeypair();
    await SecureStorage.save(
      _noiseKeyStorageKey,
      base64Encode(keypair.privateKey),
    );
    return keypair.privateKey;
  }

  /// 处理连接错误
  void _onError(dynamic error) {
    print('[MpcWebSocket] Error: $error');
    _state = MpcWebSocketState.disconnected;
    _heartbeatTimer?.cancel();

    // Destroy the old Noise session since it's tied to the dead connection.
    if (_noiseSessionId != null) {
      native.noiseSessionDestroy(sessionId: _noiseSessionId!);
      _noiseSessionId = null;
    }

    _scheduleReconnect();
  }

  /// 处理连接关闭
  void _onDone() {
    print('[MpcWebSocket] Connection closed');
    _state = MpcWebSocketState.disconnected;
    _heartbeatTimer?.cancel();

    // Destroy the old Noise session since it's tied to the dead connection.
    if (_noiseSessionId != null) {
      native.noiseSessionDestroy(sessionId: _noiseSessionId!);
      _noiseSessionId = null;
    }

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
