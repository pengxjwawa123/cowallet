import 'dart:async';
import 'dart:convert';
import 'package:firebase_core/firebase_core.dart';
import 'package:firebase_messaging/firebase_messaging.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_local_notifications/flutter_local_notifications.dart';
import 'package:dio/dio.dart';

/// Push notification service for MPC signing requests
/// Handles FCM token registration, foreground/background message handling,
/// and navigation to approval screens.
class PushService {
  static final PushService _instance = PushService._internal();
  factory PushService() => _instance;
  PushService._internal();

  final FirebaseMessaging _messaging = FirebaseMessaging.instance;
  final FlutterLocalNotificationsPlugin _localNotifications =
      FlutterLocalNotificationsPlugin();

  String? _fcmToken;
  String? get fcmToken => _fcmToken;

  final _messageController = StreamController<Map<String, dynamic>>.broadcast();
  Stream<Map<String, dynamic>> get onMessage => _messageController.stream;

  bool _initialized = false;

  /// Initialize Firebase and request notification permissions
  Future<void> initialize({
    required String apiBaseUrl,
    required String authToken,
    String? deviceId,
  }) async {
    if (_initialized) return;

    try {
      // Initialize Firebase
      await Firebase.initializeApp();

      // Request notification permissions (iOS requires explicit request)
      final settings = await _messaging.requestPermission(
        alert: true,
        badge: true,
        sound: true,
        provisional: false,
      );

      if (settings.authorizationStatus == AuthorizationStatus.denied) {
        debugPrint('[PushService] Notification permissions denied');
        return;
      }

      // Initialize local notifications for foreground display
      const androidSettings = AndroidInitializationSettings('@mipmap/ic_launcher');
      const iosSettings = DarwinInitializationSettings(
        requestAlertPermission: true,
        requestBadgePermission: true,
        requestSoundPermission: true,
      );
      await _localNotifications.initialize(
        const InitializationSettings(android: androidSettings, iOS: iosSettings),
        onDidReceiveNotificationResponse: _onNotificationTap,
      );

      // Get FCM token
      _fcmToken = await _messaging.getToken();
      if (_fcmToken != null) {
        debugPrint('[PushService] FCM token: $_fcmToken');
        await _registerToken(apiBaseUrl, authToken, _fcmToken!, deviceId);
      }

      // Listen for token refresh
      _messaging.onTokenRefresh.listen((newToken) {
        _fcmToken = newToken;
        _registerToken(apiBaseUrl, authToken, newToken, deviceId);
      });

      // Handle foreground messages
      FirebaseMessaging.onMessage.listen(_handleForegroundMessage);

      // Handle notification taps when app is in background/terminated
      FirebaseMessaging.onMessageOpenedApp.listen(_handleMessageOpenedApp);

      _initialized = true;
      debugPrint('[PushService] Initialized successfully');
    } catch (e) {
      debugPrint('[PushService] Initialization failed: $e');
    }
  }

  /// Register FCM token with backend
  Future<void> _registerToken(
    String apiBaseUrl,
    String authToken,
    String token,
    String? deviceId,
  ) async {
    try {
      final dio = Dio(BaseOptions(
        baseUrl: apiBaseUrl,
        headers: {'Authorization': 'Bearer $authToken'},
        connectTimeout: const Duration(seconds: 10),
        receiveTimeout: const Duration(seconds: 10),
      ));

      await dio.post('/api/v1/push/register', data: {
        'token': token,
        'platform': defaultTargetPlatform == TargetPlatform.iOS ? 'ios' : 'android',
        'device_id': deviceId ?? 'unknown',
      });

      debugPrint('[PushService] Token registered successfully');
    } catch (e) {
      debugPrint('[PushService] Token registration failed: $e');
    }
  }

  /// Handle foreground messages (show local notification)
  void _handleForegroundMessage(RemoteMessage message) {
    debugPrint('[PushService] Foreground message: ${message.data}');

    final data = message.data;
    final type = data['type'] as String?;

    if (type == 'mpc_sign_request') {
      _showLocalNotification(
        title: 'Signature Request',
        body: 'Approve transaction: ${data['amount'] ?? 'N/A'} to ${data['to'] ?? 'N/A'}',
        payload: jsonEncode(data),
      );
    }

    _messageController.add(data);
  }

  /// Handle message tap (navigate to approval screen)
  void _handleMessageOpenedApp(RemoteMessage message) {
    debugPrint('[PushService] Message opened: ${message.data}');
    _messageController.add(message.data);
  }

  /// Handle local notification tap
  void _onNotificationTap(NotificationResponse response) {
    if (response.payload != null) {
      final data = jsonDecode(response.payload!) as Map<String, dynamic>;
      _messageController.add(data);
    }
  }

  /// Show local notification for foreground messages
  Future<void> _showLocalNotification({
    required String title,
    required String body,
    String? payload,
  }) async {
    const androidDetails = AndroidNotificationDetails(
      'mpc_signing',
      'MPC Signing Requests',
      channelDescription: 'Notifications for MPC transaction signing requests',
      importance: Importance.high,
      priority: Priority.high,
      showWhen: true,
    );

    const iosDetails = DarwinNotificationDetails(
      presentAlert: true,
      presentBadge: true,
      presentSound: true,
    );

    await _localNotifications.show(
      DateTime.now().millisecondsSinceEpoch ~/ 1000,
      title,
      body,
      const NotificationDetails(android: androidDetails, iOS: iosDetails),
      payload: payload,
    );
  }

  /// Clean up resources
  void dispose() {
    _messageController.close();
  }
}

/// Background message handler (must be top-level function)
@pragma('vm:entry-point')
Future<void> firebaseMessagingBackgroundHandler(RemoteMessage message) async {
  await Firebase.initializeApp();
  debugPrint('[PushService] Background message: ${message.data}');

  // For MPC signing requests, we can show a notification even in background
  // The actual handling will occur when the user taps it
  if (message.data['type'] == 'mpc_sign_request') {
    // Background notifications are handled by FCM automatically on iOS/Android
    debugPrint('[PushService] MPC signing request received in background');
  }
}
