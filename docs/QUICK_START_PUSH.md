# Quick Start: Push Notifications

This is a quick reference for setting up push notifications. For detailed documentation, see `PUSH_NOTIFICATIONS.md`.

## 1. Firebase Setup (One-time)

### Get Firebase Config Files

1. Go to [Firebase Console](https://console.firebase.google.com/)
2. Create/select project
3. Add iOS app:
   - Download `GoogleService-Info.plist`
   - Copy to `mobile/ios/Runner/GoogleService-Info.plist`
4. Add Android app:
   - Download `google-services.json`
   - Copy to `mobile/android/app/google-services.json`

### Get FCM Server Key

1. Firebase Console → Project Settings → Cloud Messaging
2. Copy "Server key" (under Cloud Messaging API Legacy)
3. Add to backend `.env`:
   ```bash
   FCM_SERVER_KEY=AAAA...your_key_here
   ```

## 2. iOS Setup

```bash
cd mobile/ios
pod install
```

Then in Xcode:
- Open `Runner.xcworkspace`
- Select Runner target → Signing & Capabilities
- Add **Push Notifications** capability
- Add **Background Modes** → enable "Remote notifications"

## 3. Android Setup

Already configured in `build.gradle`. Just ensure `google-services.json` is in place.

## 4. Backend Setup

```bash
# Run migration
cd backend
sqlx migrate run --source migrations

# Add FCM key to .env
echo "FCM_SERVER_KEY=your_key_here" >> .env

# Start server
cargo run --bin api-server
```

## 5. Flutter Integration

### In `main.dart`:

```dart
import 'package:firebase_core/firebase_core.dart';
import 'package:firebase_messaging/firebase_messaging.dart';
import 'services/push_service.dart';

@pragma('vm:entry-point')
Future<void> _firebaseMessagingBackgroundHandler(RemoteMessage message) async {
  await Firebase.initializeApp();
  print('Background message: ${message.data}');
}

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  
  await Firebase.initializeApp();
  FirebaseMessaging.onBackgroundMessage(_firebaseMessagingBackgroundHandler);
  
  runApp(MyApp());
}
```

### After user login:

```dart
final pushService = PushService();
await pushService.initialize(
  apiBaseUrl: 'http://localhost:3000',
  authToken: yourJwtToken,
  deviceId: 'test_device',
);

// Listen for MPC signing requests
pushService.onMessage.listen((data) {
  if (data['type'] == 'mpc_sign_request') {
    final sessionId = data['session_id'];
    Navigator.pushNamed(context, '/mpc-approval', arguments: sessionId);
  }
});
```

## 6. Send Push from Backend

```rust
use crate::routes::push::send_mpc_signing_notification;

// In your MPC endpoint:
send_mpc_signing_notification(
    db,
    &http_client,
    user_id,
    "session_123",
    "0.1 ETH",
    "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb",
).await?;
```

## 7. Test

### Test token registration:
```bash
curl -X POST http://localhost:3000/api/v1/push/register \
  -H "Authorization: Bearer YOUR_JWT" \
  -H "Content-Type: application/json" \
  -d '{"token":"test_token","platform":"ios","device_id":"test_device"}'
```

### Test sending push:
```bash
curl -X POST http://localhost:3000/api/v1/push/send \
  -H "Content-Type: application/json" \
  -d '{
    "user_id": 1,
    "title": "Test Push",
    "body": "This is a test",
    "data": {
      "type": "mpc_sign_request",
      "session_id": "test",
      "amount": "0.1 ETH",
      "to": "0x742d..."
    }
  }'
```

## Common Issues

### iOS: "No valid 'aps-environment' entitlement"
- Enable Push Notifications capability in Xcode
- Test on physical device (not simulator)

### Backend: "FCM not configured"
- Verify `FCM_SERVER_KEY` is set in `.env`
- Restart API server after adding key

### Flutter: Token not generated
- Check Firebase initialization completed
- Request permissions: `FirebaseMessaging.instance.requestPermission()`
- Check device logs for errors

## Files Checklist

- [ ] `mobile/ios/Runner/GoogleService-Info.plist` (from Firebase)
- [ ] `mobile/android/app/google-services.json` (from Firebase)
- [ ] `FCM_SERVER_KEY` in backend `.env`
- [ ] Pods installed (`cd mobile/ios && pod install`)
- [ ] Migration run (`sqlx migrate run`)
- [ ] Push capability enabled in Xcode

## Architecture Overview

```
┌─────────────┐         ┌─────────────┐         ┌─────────────┐
│   Flutter   │────1───▶│   Backend   │────2───▶│     FCM     │
│  (Register) │         │  (Store)    │         │   (Apple)   │
└─────────────┘         └─────────────┘         └─────────────┘
                               │                       │
                               │                       │
                           3. MPC Event           4. Push
                               │                       │
                               ▼                       ▼
                        ┌─────────────┐         ┌─────────────┐
                        │   Backend   │         │   Device    │
                        │  (Trigger)  │         │  (Receive)  │
                        └─────────────┘         └─────────────┘
```

1. App registers FCM token with backend
2. Backend stores token in database
3. MPC signing request triggers push send
4. FCM delivers to device → app shows notification

## Next Steps

- Implement approval screen UI
- Add push notification analytics
- Handle token expiration/refresh
- Add retry logic for failed sends
- Monitor FCM delivery metrics in Firebase Console

## Resources

- Full docs: `docs/PUSH_NOTIFICATIONS.md`
- Example usage: `mobile/lib/services/push_service_example.dart`
- Backend route: `backend/api-server/src/routes/push.rs`
