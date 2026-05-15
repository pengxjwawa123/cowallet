import 'dart:async';
import 'dart:convert';

import 'package:firebase_core/firebase_core.dart';
import 'package:firebase_messaging/firebase_messaging.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_local_notifications/flutter_local_notifications.dart';

import '../l10n/strings.dart';
import '../network/dio_client.dart';
import '../utils/secure_storage.dart';

/// Push notification types received from the backend.
abstract class PushType {
  static const txConfirmed = 'tx_confirmed';
  static const txFailed = 'tx_failed';
  static const securityAlert = 'security_alert';
  static const mpcSignRequest = 'mpc_sign_request';
}

/// Remote push notification service (FCM for Android, APNs for iOS).
///
/// Handles:
/// - Permission requests
/// - FCM/APNs token registration with backend
/// - Foreground, background, and terminated-state message handling
/// - Routing push data to appropriate local notification channels
/// - Navigation on notification tap
class PushService {
  static final PushService _instance = PushService._internal();
  factory PushService() => _instance;
  PushService._internal();

  FirebaseMessaging? _messaging;
  final FlutterLocalNotificationsPlugin _localNotifications =
      FlutterLocalNotificationsPlugin();

  String? _fcmToken;
  String? get fcmToken => _fcmToken;

  bool _initialized = false;
  bool get isInitialized => _initialized;

  /// Stream of push message payloads for in-app consumers (e.g. navigation).
  final _messageController = StreamController<Map<String, dynamic>>.broadcast();
  Stream<Map<String, dynamic>> get onMessage => _messageController.stream;

  /// Callback for notification tap navigation. Set by the app shell.
  void Function(Map<String, dynamic> data)? onNotificationTap;

  // Android notification channels matching NotificationService channels.
  static const _txChannelId = 'cowallet_transactions';
  static const _txChannelName = 'Transactions';
  static const _txChannelDesc = 'Transaction confirmations and failures';

  static const _securityChannelId = 'cowallet_security';
  static const _securityChannelName = 'Security Alerts';
  static const _securityChannelDesc = 'Security alerts and warnings';

  static const _mpcChannelId = 'cowallet_mpc';
  static const _mpcChannelName = 'MPC Signing Requests';
  static const _mpcChannelDesc = 'Notifications for MPC transaction signing';

  /// Initialize the push notification service.
  ///
  /// Gracefully no-ops if Firebase is not configured (e.g. missing
  /// GoogleService-Info.plist / google-services.json). This allows the app
  /// to run without Firebase during development.
  Future<void> init() async {
    if (_initialized) return;

    try {
      await Firebase.initializeApp();
    } catch (e) {
      debugPrint('[PushService] Firebase not configured, skipping push init: $e');
      return;
    }

    try {
      _messaging = FirebaseMessaging.instance;

      // Request permissions (iOS requires explicit prompt)
      final settings = await _messaging!.requestPermission(
        alert: true,
        badge: true,
        sound: true,
        provisional: false,
      );

      if (settings.authorizationStatus == AuthorizationStatus.denied) {
        debugPrint('[PushService] Notification permissions denied by user');
        return;
      }

      debugPrint(
          '[PushService] Permission status: ${settings.authorizationStatus}');

      // Initialize local notifications for foreground display
      await _initLocalNotifications();

      // Set background message handler
      FirebaseMessaging.onBackgroundMessage(firebaseMessagingBackgroundHandler);

      // Get initial FCM token
      _fcmToken = await _messaging!.getToken();
      debugPrint('[PushService] FCM token obtained: ${_fcmToken?.substring(0, 20)}...');

      // Register token with backend (best-effort, non-blocking)
      if (_fcmToken != null) {
        _registerTokenWithBackend(_fcmToken!);
      }

      // Listen for token refresh
      _messaging!.onTokenRefresh.listen((newToken) {
        debugPrint('[PushService] FCM token refreshed');
        _fcmToken = newToken;
        _registerTokenWithBackend(newToken);
      });

      // Handle foreground messages
      FirebaseMessaging.onMessage.listen(_handleForegroundMessage);

      // Handle notification tap when app is in background/terminated
      FirebaseMessaging.onMessageOpenedApp.listen(_handleMessageOpenedApp);

      // Check if app was opened from a terminated state via notification
      final initialMessage = await _messaging!.getInitialMessage();
      if (initialMessage != null) {
        // Delay to allow the app to fully initialize before navigating
        Future.delayed(const Duration(milliseconds: 500), () {
          _handleMessageOpenedApp(initialMessage);
        });
      }

      _initialized = true;
      debugPrint('[PushService] Initialized successfully');
    } catch (e) {
      debugPrint('[PushService] Initialization error: $e');
    }
  }

  /// Initialize local notification plugin and Android channels.
  Future<void> _initLocalNotifications() async {
    const androidSettings =
        AndroidInitializationSettings('@mipmap/ic_launcher');
    const iosSettings = DarwinInitializationSettings(
      requestAlertPermission: true,
      requestBadgePermission: true,
      requestSoundPermission: true,
    );

    await _localNotifications.initialize(
      const InitializationSettings(android: androidSettings, iOS: iosSettings),
      onDidReceiveNotificationResponse: _onLocalNotificationTap,
    );

    // Create Android notification channels
    final androidPlugin = _localNotifications
        .resolvePlatformSpecificImplementation<
            AndroidFlutterLocalNotificationsPlugin>();
    if (androidPlugin != null) {
      await androidPlugin.createNotificationChannel(
        const AndroidNotificationChannel(
          _txChannelId,
          _txChannelName,
          description: _txChannelDesc,
          importance: Importance.high,
        ),
      );
      await androidPlugin.createNotificationChannel(
        const AndroidNotificationChannel(
          _securityChannelId,
          _securityChannelName,
          description: _securityChannelDesc,
          importance: Importance.max,
        ),
      );
      await androidPlugin.createNotificationChannel(
        const AndroidNotificationChannel(
          _mpcChannelId,
          _mpcChannelName,
          description: _mpcChannelDesc,
          importance: Importance.high,
        ),
      );
    }
  }

  /// Register the FCM token with the backend push/register endpoint.
  Future<void> _registerTokenWithBackend(String token) async {
    try {
      final deviceId = await SecureStorage.getDeviceId();

      final result = await DioClient.post(
        '/push/register',
        data: {
          'token': token,
          'platform':
              defaultTargetPlatform == TargetPlatform.iOS ? 'ios' : 'android',
          'device_id': deviceId ?? 'unknown',
        },
      );

      if (result.isSuccess) {
        debugPrint('[PushService] Token registered with backend');
      } else {
        debugPrint(
            '[PushService] Token registration failed: ${result.errorMessage}');
      }
    } catch (e) {
      debugPrint('[PushService] Token registration error: $e');
    }
  }

  /// Re-register token after login (call when auth token becomes available).
  Future<void> reregisterToken() async {
    if (_fcmToken != null) {
      await _registerTokenWithBackend(_fcmToken!);
    }
  }

  /// Handle a foreground push message: show local notification + emit to stream.
  void _handleForegroundMessage(RemoteMessage message) {
    debugPrint('[PushService] Foreground message: ${message.data}');

    final data = message.data;
    final type = data['type'] as String?;

    switch (type) {
      case PushType.txConfirmed:
        _showTxConfirmedNotification(data);
        break;
      case PushType.txFailed:
        _showTxFailedNotification(data);
        break;
      case PushType.securityAlert:
        _showSecurityAlertNotification(data);
        break;
      case PushType.mpcSignRequest:
        _showMpcSignRequestNotification(data);
        break;
      default:
        // Show the FCM notification title/body if present
        final notification = message.notification;
        if (notification != null) {
          _showGenericNotification(
            title: notification.title ?? 'CoWallet',
            body: notification.body ?? '',
            payload: jsonEncode(data),
            channelId: _txChannelId,
          );
        }
    }

    _messageController.add(data);
  }

  /// Handle notification tap when app was in background.
  void _handleMessageOpenedApp(RemoteMessage message) {
    debugPrint('[PushService] Message opened app: ${message.data}');
    final data = message.data;
    _messageController.add(data);
    onNotificationTap?.call(data);
  }

  /// Handle tap on a local notification shown in foreground.
  void _onLocalNotificationTap(NotificationResponse response) {
    if (response.payload != null && response.payload!.isNotEmpty) {
      try {
        final data = jsonDecode(response.payload!) as Map<String, dynamic>;
        _messageController.add(data);
        onNotificationTap?.call(data);
      } catch (_) {}
    }
  }

  // ─── Notification display helpers ─────────────────────────────────────────

  void _showTxConfirmedNotification(Map<String, dynamic> data) {
    final txHash = data['tx_hash'] ?? '';
    final amount = data['amount'] ?? '';
    final token = data['token'] ?? '';
    final shortHash = txHash.length >= 16
        ? '${txHash.substring(0, 10)}...${txHash.substring(txHash.length - 6)}'
        : txHash;

    _showGenericNotification(
      title: S.notifTxConfirmedTitle,
      body: S.notifTxConfirmedBody(amount, token, shortHash),
      payload: jsonEncode(data),
      channelId: _txChannelId,
      importance: Importance.high,
      priority: Priority.high,
    );
  }

  void _showTxFailedNotification(Map<String, dynamic> data) {
    final txHash = data['tx_hash'] ?? '';
    final reason = data['reason'] ?? 'unknown';
    final shortHash = txHash.length >= 16
        ? '${txHash.substring(0, 10)}...${txHash.substring(txHash.length - 6)}'
        : txHash;

    _showGenericNotification(
      title: S.notifTxFailedTitle,
      body: S.notifTxFailedBody(shortHash, reason),
      payload: jsonEncode(data),
      channelId: _txChannelId,
      importance: Importance.high,
      priority: Priority.high,
    );
  }

  void _showSecurityAlertNotification(Map<String, dynamic> data) {
    final title = data['title'] as String? ?? S.notifSecurityAlertTitle;
    final body = data['message'] as String? ?? data['body'] as String? ?? '';

    _showGenericNotification(
      title: title,
      body: body,
      payload: jsonEncode(data),
      channelId: _securityChannelId,
      importance: Importance.max,
      priority: Priority.max,
    );
  }

  void _showMpcSignRequestNotification(Map<String, dynamic> data) {
    final amount = data['amount'] ?? 'N/A';
    final to = data['to'] ?? 'N/A';

    _showGenericNotification(
      title: 'Signature Request',
      body: 'Approve transaction: $amount to $to',
      payload: jsonEncode(data),
      channelId: _mpcChannelId,
      importance: Importance.high,
      priority: Priority.high,
    );
  }

  Future<void> _showGenericNotification({
    required String title,
    required String body,
    String? payload,
    required String channelId,
    Importance importance = Importance.high,
    Priority priority = Priority.high,
  }) async {
    final androidDetails = AndroidNotificationDetails(
      channelId,
      channelId == _txChannelId
          ? _txChannelName
          : channelId == _securityChannelId
              ? _securityChannelName
              : _mpcChannelName,
      channelDescription: channelId == _txChannelId
          ? _txChannelDesc
          : channelId == _securityChannelId
              ? _securityChannelDesc
              : _mpcChannelDesc,
      importance: importance,
      priority: priority,
      icon: '@mipmap/ic_launcher',
      showWhen: true,
    );

    const iosDetails = DarwinNotificationDetails(
      presentAlert: true,
      presentBadge: true,
      presentSound: true,
    );

    await _localNotifications.show(
      DateTime.now().millisecondsSinceEpoch % 0x7FFFFFFF,
      title,
      body,
      NotificationDetails(android: androidDetails, iOS: iosDetails),
      payload: payload,
    );
  }

  /// Clean up resources.
  void dispose() {
    _messageController.close();
  }
}

/// Top-level background message handler (must be a top-level function).
///
/// Called when a push message arrives while the app is terminated or in
/// background. FCM automatically shows the notification from the `notification`
/// payload; this handler processes the `data` payload for bookkeeping.
@pragma('vm:entry-point')
Future<void> firebaseMessagingBackgroundHandler(RemoteMessage message) async {
  // Ensure Firebase is initialized in the background isolate.
  try {
    await Firebase.initializeApp();
  } catch (_) {}

  debugPrint('[PushService] Background message: ${message.data}');

  // Background data-only messages can be processed here if needed.
  // For now, FCM handles showing the notification automatically when
  // the `notification` field is present in the push payload.
}
