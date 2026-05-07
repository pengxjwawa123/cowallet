import 'dart:async';
import 'dart:convert';
import 'package:web_socket_channel/web_socket_channel.dart';

import '../config/api_config.dart';
import '../utils/secure_storage.dart';

/// MPC协议消息
class MpcMessage {
  final int fromParty;
  final int toParty;
  final int round;
  final List<int> payload;

  const MpcMessage({
    required this.fromParty,
    required this.toParty,
    required this.round,
    required this.payload,
  });

  factory MpcMessage.fromJson(Map<String, dynamic> json) {
    return MpcMessage(
      fromParty: json['from_party'] as int,
      toParty: json['to_party'] as int,
      round: json['round'] as int,
      payload: (json['payload'] as List<dynamic>).cast<int>(),
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
  reconnecting,
}

/// 管理MPC会话的WebSocket连接，用于实时传输MPC协议消息。
/// 连接地址: ws://host/api/v1/mpc/session/{sessionId}/ws?party={partyIndex}&token={jwt}
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

  static const int _maxReconnectAttempts = 5;
  static const Duration _heartbeatInterval = Duration(seconds: 30);

  MpcWebSocket({
    required this.sessionId,
    required this.partyIndex,
  });

  /// 当前连接状态
  MpcWebSocketState get state => _state;

  /// 是否已连接
  bool get isConnected => _state == MpcWebSocketState.connected;

  /// 消息流，监听此流以接收MPC消息
  Stream<MpcMessage> get messages {
    _messageController ??= StreamController<MpcMessage>.broadcast();
    return _messageController!.stream;
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
  Future<void> connect() async {
    if (_state == MpcWebSocketState.connected ||
        _state == MpcWebSocketState.connecting) {
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

      _state = MpcWebSocketState.connected;
      _reconnectAttempts = 0;
      print('[MpcWebSocket] Connected successfully');

      // 监听消息
      _subscription = _channel!.stream.listen(
        _onMessage,
        onError: _onError,
        onDone: _onDone,
      );

      // 启动心跳
      _startHeartbeat();
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

  /// 发送MPC消息
  /// [message] 要发送的MPC消息
  void send(MpcMessage message) {
    if (_state != MpcWebSocketState.connected || _channel == null) {
      print('[MpcWebSocket] Cannot send: not connected');
      return;
    }

    String jsonStr = jsonEncode(message.toJson());
    _channel!.sink.add(jsonStr);
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

  /// 处理收到的消息
  void _onMessage(dynamic data) {
    try {
      Map<String, dynamic> json;
      if (data is String) {
        json = jsonDecode(data) as Map<String, dynamic>;
      } else {
        // 二进制消息，尝试UTF-8解码
        json = jsonDecode(utf8.decode(data as List<int>))
            as Map<String, dynamic>;
      }

      // 忽略心跳pong响应
      if (json.containsKey('type') && json['type'] == 'pong') {
        return;
      }

      MpcMessage message = MpcMessage.fromJson(json);
      _messageController?.add(message);
    } catch (e) {
      print('[MpcWebSocket] Error parsing message: $e');
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
    if (_reconnectAttempts >= _maxReconnectAttempts) {
      print('[MpcWebSocket] Max reconnect attempts reached');
      _messageController?.addError(
        Exception('WebSocket connection failed after $_maxReconnectAttempts attempts'),
      );
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
