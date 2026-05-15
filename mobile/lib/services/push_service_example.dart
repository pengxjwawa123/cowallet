/// Example usage of PushService in the app.
///
/// PushService is automatically initialized via `Services.init()` in the
/// service locator. No manual initialization is needed.
///
/// ## How it works
///
/// 1. On app start, `Services.push.init()` initializes Firebase, requests
///    permissions, obtains the FCM token, and registers it with the backend.
///
/// 2. When a push notification arrives:
///    - **Foreground**: a local notification is shown using the appropriate
///      channel (transactions, security, MPC signing).
///    - **Background/Terminated**: FCM shows the notification automatically.
///      When tapped, the app navigates to the relevant screen.
///
/// 3. After login/token refresh, call `Services.push.reregisterToken()` to
///    ensure the backend has the current FCM token associated with the
///    authenticated user.
///
/// ## Listening for push messages in-app
///
/// ```dart
/// Services.push.onMessage.listen((data) {
///   final type = data['type'];
///   if (type == PushType.txConfirmed) {
///     // Refresh balance, show success indicator, etc.
///   }
/// });
/// ```
///
/// ## Backend push payload format
///
/// The backend sends data-only messages with these fields:
///
/// ```json
/// {
///   "type": "tx_confirmed",      // or tx_failed, security_alert, mpc_sign_request
///   "tx_hash": "0xabc...",
///   "amount": "0.1",
///   "token": "ETH",
///   "chain_id": "8453"
/// }
/// ```
///
/// For security alerts:
/// ```json
/// {
///   "type": "security_alert",
///   "title": "Suspicious Activity",
///   "message": "Unusual login attempt detected"
/// }
/// ```
///
/// For MPC signing requests:
/// ```json
/// {
///   "type": "mpc_sign_request",
///   "session_id": "abc-123",
///   "amount": "0.5 ETH",
///   "to": "0x742d..."
/// }
/// ```

library;
