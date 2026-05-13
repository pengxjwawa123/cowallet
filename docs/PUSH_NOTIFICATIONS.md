# Push Notifications Setup Guide

This guide explains how to set up push notifications for MPC signing requests in cowallet.

## Overview

The push notification system uses Firebase Cloud Messaging (FCM) to send real-time alerts to mobile devices when an MPC signing request requires user approval.

**Flow:**
1. Mobile app registers FCM token with backend on startup
2. Backend stores token in `push_tokens` table
3. When MPC session needs approval, backend sends push via FCM
4. User taps notification → app navigates to approval screen

## Architecture

### Backend Components

- **Migration**: `backend/migrations/011_push_tokens.sql` — Database table for storing FCM tokens
- **Route**: `backend/api-server/src/routes/push.rs` — API endpoints for token registration and sending pushes
- **Helper**: `send_mpc_signing_notification()` function for sending MPC signing requests

### Mobile Components

- **Service**: `mobile/lib/services/push_service.dart` — Flutter service for FCM integration
- **iOS Native**: `mobile/ios/Runner/AppDelegate.swift` — Firebase iOS initialization
- **Dependencies**: `firebase_core`, `firebase_messaging`, `flutter_local_notifications`

## Setup Instructions

### 1. Firebase Project Setup

1. Go to [Firebase Console](https://console.firebase.google.com/)
2. Create a new project or select existing one
3. Add iOS app:
   - Bundle ID: `com.cowallet` (match your app's bundle ID)
   - Download `GoogleService-Info.plist`
   - Place in `mobile/ios/Runner/GoogleService-Info.plist`
4. Add Android app (if needed):
   - Package name: `com.cowallet`
   - Download `google-services.json`
   - Place in `mobile/android/app/google-services.json`

### 2. Get FCM Server Key

1. In Firebase Console, go to **Project Settings** (gear icon)
2. Navigate to **Cloud Messaging** tab
3. Copy **Server key** under "Cloud Messaging API (Legacy)"
4. Add to `.env`:
   ```bash
   FCM_SERVER_KEY=your_server_key_here
   ```

**Note**: The implementation uses FCM Legacy HTTP API. For new projects, consider migrating to FCM HTTP v1 API.

### 3. iOS Configuration

#### Add Firebase SDK to Podfile

Edit `mobile/ios/Podfile` and ensure you have:

```ruby
target 'Runner' do
  use_frameworks!
  use_modular_headers!

  flutter_install_all_ios_pods File.dirname(File.realpath(__FILE__))

  # Firebase dependencies
  pod 'FirebaseCore'
  pod 'FirebaseMessaging'
end
```

#### Install Pods

```bash
cd mobile/ios
pod install
```

#### Add Push Notification Capability

1. Open `mobile/ios/Runner.xcworkspace` in Xcode
2. Select the Runner target
3. Go to **Signing & Capabilities**
4. Click **+ Capability**
5. Add **Push Notifications**
6. Add **Background Modes** and enable:
   - Remote notifications
   - Background fetch

#### Update Info.plist

Add Firebase configuration to `mobile/ios/Runner/Info.plist`:

```xml
<key>FirebaseAppDelegateProxyEnabled</key>
<false/>
```

### 4. Android Configuration

#### Add google-services Plugin

Edit `mobile/android/build.gradle`:

```gradle
buildscript {
    dependencies {
        // ... existing dependencies
        classpath 'com.google.gms:google-services:4.4.0'
    }
}
```

Edit `mobile/android/app/build.gradle`:

```gradle
apply plugin: 'com.google.gms.google-services'

android {
    defaultConfig {
        // ... existing config
        minSdkVersion 21  // Required for FCM
    }
}
```

#### Add Permissions

The `firebase_messaging` plugin handles permissions automatically.

### 5. Flutter Integration

#### Initialize in main.dart

```dart
import 'package:firebase_core/firebase_core.dart';
import 'services/push_service.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();

  // Initialize Firebase
  await Firebase.initializeApp();

  // Register FCM background handler
  FirebaseMessaging.onBackgroundMessage(firebaseMessagingBackgroundHandler);

  runApp(MyApp());
}
```

#### Initialize PushService After Login

```dart
// After successful authentication
final pushService = PushService();
await pushService.initialize(
  apiBaseUrl: 'https://api.cowallet.com',
  authToken: userToken,
  deviceId: await getDeviceId(),
);

// Listen for MPC signing requests
pushService.onMessage.listen((data) {
  if (data['type'] == 'mpc_sign_request') {
    Navigator.pushNamed(context, '/mpc-approval', arguments: data);
  }
});
```

### 6. Backend Integration

#### Send Push When MPC Session Starts

```rust
use crate::routes::push::send_mpc_signing_notification;

// In your MPC session creation endpoint:
async fn create_mpc_session(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Json(req): Json<CreateSessionRequest>,
) -> Result<impl IntoResponse> {
    let db = state.require_db()?;

    // Create MPC session...
    let session_id = create_session(&db, &req).await?;

    // Send push notification
    let _ = send_mpc_signing_notification(
        &db,
        &state.http,
        auth_user.user_id,
        &session_id,
        &req.amount,
        &req.to_address,
    ).await;

    Ok(Json(CreateSessionResponse { session_id }))
}
```

## API Endpoints

### POST /api/v1/push/register

Register or update FCM token for the authenticated user.

**Auth**: Required (JWT)

**Request:**
```json
{
  "token": "fcm_token_here",
  "platform": "ios",  // or "android"
  "device_id": "device_unique_id"
}
```

**Response:**
```json
{
  "success": true
}
```

### POST /api/v1/push/send

Send push notification to a user's devices (internal use only).

**Auth**: None (should be IP-restricted in production)

**Request:**
```json
{
  "user_id": 123,
  "title": "Signature Request",
  "body": "Approve transaction: 0.1 ETH to 0x742d...",
  "data": {
    "type": "mpc_sign_request",
    "session_id": "session_abc123",
    "amount": "0.1 ETH",
    "to": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb"
  }
}
```

**Response:**
```json
{
  "success": true,
  "sent_count": 2
}
```

## Message Payload Format

MPC signing requests use this payload structure:

```json
{
  "type": "mpc_sign_request",
  "session_id": "session_abc123",
  "amount": "0.1 ETH",
  "to": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb"
}
```

## Testing

### Test Token Registration

```bash
curl -X POST http://localhost:3000/api/v1/push/register \
  -H "Authorization: Bearer YOUR_JWT_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "token": "test_fcm_token",
    "platform": "ios",
    "device_id": "test_device_001"
  }'
```

### Test Sending Push

```bash
curl -X POST http://localhost:3000/api/v1/push/send \
  -H "Content-Type: application/json" \
  -d '{
    "user_id": 1,
    "title": "Test Notification",
    "body": "This is a test",
    "data": {
      "type": "mpc_sign_request",
      "session_id": "test_session",
      "amount": "0.1 ETH",
      "to": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb"
    }
  }'
```

## Database Schema

```sql
CREATE TABLE push_tokens (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token TEXT NOT NULL UNIQUE,
    platform TEXT NOT NULL CHECK (platform IN ('ios', 'android')),
    device_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

## Security Considerations

1. **Token Storage**: FCM tokens are stored in plaintext (they're not secrets, but device identifiers)
2. **Send Endpoint**: The `/api/v1/push/send` endpoint should be restricted to internal use:
   - Add IP whitelist for backend services
   - Or add internal auth token
   - Or remove it and only call `send_mpc_signing_notification()` from Rust
3. **Token Expiration**: Tokens can expire or become invalid; handle `NotRegistered` errors from FCM
4. **User Privacy**: Users can revoke notification permissions; gracefully handle permission denials

## Troubleshooting

### iOS: Notifications Not Arriving

1. Check Push Notifications capability is enabled in Xcode
2. Verify `GoogleService-Info.plist` is in the correct location
3. Check device has notification permissions granted
4. Test on a physical device (push doesn't work on simulator)

### Android: Notifications Not Arriving

1. Verify `google-services.json` is in `android/app/`
2. Check `minSdkVersion` is 21 or higher
3. Ensure Google Play Services is installed on device

### FCM Token Not Generated

1. Check Firebase initialization completed successfully
2. Verify notification permissions were granted
3. Check app logs for FCM registration errors

### Backend: Push Send Fails

1. Verify `FCM_SERVER_KEY` is set correctly in `.env`
2. Check FCM API is enabled in Firebase Console
3. Review backend logs for HTTP error responses from FCM

## Migration from APNs Direct to FCM

If you previously used APNs directly for iOS:

1. FCM handles APNs certificates automatically
2. Remove old APNs token handling code
3. Update push payloads to FCM format
4. Test thoroughly on both iOS and Android

## References

- [Firebase Cloud Messaging Documentation](https://firebase.google.com/docs/cloud-messaging)
- [flutter_firebase_messaging Plugin](https://pub.dev/packages/firebase_messaging)
- [FCM HTTP v1 API Migration Guide](https://firebase.google.com/docs/cloud-messaging/migrate-v1)
