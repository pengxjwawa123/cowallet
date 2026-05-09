import 'dart:io';

import 'package:flutter_local_notifications/flutter_local_notifications.dart';

/// Local push notification service for transaction events and security alerts.
///
/// Channels:
///   - cowallet_transactions: tx confirmed / failed
///   - cowallet_security: security alerts
class NotificationService {
  final FlutterLocalNotificationsPlugin _plugin =
      FlutterLocalNotificationsPlugin();

  bool _initialized = false;

  // Android notification channel definitions
  static const _txChannel = AndroidNotificationChannel(
    'cowallet_transactions',
    'Transactions',
    description: 'Transaction confirmations and failures',
    importance: Importance.high,
  );

  static const _securityChannel = AndroidNotificationChannel(
    'cowallet_security',
    'Security Alerts',
    description: 'Security alerts and warnings',
    importance: Importance.max,
  );

  /// Initialize notification plugin, channels (Android), and permissions (iOS).
  Future<void> init() async {
    if (_initialized) return;

    const androidSettings =
        AndroidInitializationSettings('@mipmap/ic_launcher');

    const iosSettings = DarwinInitializationSettings(
      // Provisional notifications: delivered silently without prompting the user
      requestProvisionalPermission: true,
      requestAlertPermission: true,
      requestBadgePermission: true,
      requestSoundPermission: true,
    );

    const initSettings = InitializationSettings(
      android: androidSettings,
      iOS: iosSettings,
    );

    await _plugin.initialize(initSettings);

    // Create Android notification channels
    if (Platform.isAndroid) {
      final androidPlugin =
          _plugin.resolvePlatformSpecificImplementation<
              AndroidFlutterLocalNotificationsPlugin>();
      if (androidPlugin != null) {
        await androidPlugin.createNotificationChannel(_txChannel);
        await androidPlugin.createNotificationChannel(_securityChannel);
      }
    }

    _initialized = true;
  }

  /// Show notification for a confirmed transaction.
  Future<void> showTxConfirmed(
    String txHash,
    String amount,
    String token,
  ) async {
    if (!_initialized) return;

    final shortHash =
        '${txHash.substring(0, 10)}...${txHash.substring(txHash.length - 6)}';

    await _plugin.show(
      txHash.hashCode,
      'Transfer Confirmed',
      '$amount $token sent successfully ($shortHash)',
      NotificationDetails(
        android: AndroidNotificationDetails(
          _txChannel.id,
          _txChannel.name,
          channelDescription: _txChannel.description,
          importance: Importance.high,
          priority: Priority.high,
          icon: '@mipmap/ic_launcher',
        ),
        iOS: const DarwinNotificationDetails(
          presentAlert: true,
          presentBadge: true,
          presentSound: true,
        ),
      ),
    );
  }

  /// Show notification for a failed transaction.
  Future<void> showTxFailed(String txHash, String reason) async {
    if (!_initialized) return;

    final shortHash = txHash.length >= 16
        ? '${txHash.substring(0, 10)}...${txHash.substring(txHash.length - 6)}'
        : txHash;

    await _plugin.show(
      txHash.hashCode + 1,
      'Transfer Failed',
      'Transaction $shortHash failed: $reason',
      NotificationDetails(
        android: AndroidNotificationDetails(
          _txChannel.id,
          _txChannel.name,
          channelDescription: _txChannel.description,
          importance: Importance.high,
          priority: Priority.high,
          icon: '@mipmap/ic_launcher',
        ),
        iOS: const DarwinNotificationDetails(
          presentAlert: true,
          presentBadge: true,
          presentSound: true,
        ),
      ),
    );
  }

  /// Show a security alert notification.
  Future<void> showSecurityAlert(String title, String message) async {
    if (!_initialized) return;

    await _plugin.show(
      DateTime.now().millisecondsSinceEpoch % 0x7FFFFFFF,
      title,
      message,
      NotificationDetails(
        android: AndroidNotificationDetails(
          _securityChannel.id,
          _securityChannel.name,
          channelDescription: _securityChannel.description,
          importance: Importance.max,
          priority: Priority.max,
          icon: '@mipmap/ic_launcher',
        ),
        iOS: const DarwinNotificationDetails(
          presentAlert: true,
          presentBadge: true,
          presentSound: true,
          interruptionLevel: InterruptionLevel.critical,
        ),
      ),
    );
  }
}
