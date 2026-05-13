/// Example usage of PushService in the app
///
/// This file demonstrates how to initialize and use the push notification service
/// for handling MPC signing requests.

import 'package:flutter/material.dart';
import 'push_service.dart';

class PushServiceExample {
  /// Initialize push service on app startup (call from main.dart or app initialization)
  static Future<void> initializePushNotifications({
    required String apiBaseUrl,
    required String authToken,
    String? deviceId,
  }) async {
    final pushService = PushService();

    // Initialize with backend URL and auth token
    await pushService.initialize(
      apiBaseUrl: apiBaseUrl,
      authToken: authToken,
      deviceId: deviceId ?? 'default_device',
    );

    // Listen for push messages (MPC signing requests)
    pushService.onMessage.listen((data) {
      final type = data['type'] as String?;
      if (type == 'mpc_sign_request') {
        // Navigate to signing approval screen
        final sessionId = data['session_id'] as String?;
        final amount = data['amount'] as String?;
        final toAddress = data['to'] as String?;

        print('[Push] MPC signing request: session=$sessionId, amount=$amount, to=$toAddress');

        // TODO: Navigate to approval screen
        // Example: Navigator.of(context).pushNamed('/mpc-approval', arguments: data);
      }
    });
  }

  /// Example: Send push notification from backend when starting MPC session
  ///
  /// Call this from the backend when a new MPC signing session is created.
  ///
  /// ```rust
  /// use crate::routes::push::send_mpc_signing_notification;
  ///
  /// // In your MPC session creation endpoint:
  /// let _ = send_mpc_signing_notification(
  ///     db,
  ///     &http_client,
  ///     user_id,
  ///     &session_id,
  ///     "0.1 ETH",
  ///     "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb",
  /// ).await;
  /// ```
}
