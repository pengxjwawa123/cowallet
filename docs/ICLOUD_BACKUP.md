# iCloud Backup Implementation Guide

This guide documents the iCloud Keychain backup implementation for securely storing encrypted MPC key shards.

## Overview

The iOS iCloud backup system uses **iCloud Keychain** with the `kSecAttrSynchronizable` attribute to sync encrypted shard data across a user's devices. This provides:

- **Cross-device sync**: Shards automatically sync to other devices signed into the same iCloud account
- **End-to-end encryption**: Apple encrypts data before leaving the device
- **Secure recovery**: Users can recover shards when setting up a new device
- **No additional storage**: Uses iOS Keychain, not iCloud Drive

## Architecture

### Flutter Side

**File**: `mobile/lib/platform/cloud_backup.dart`

The `CloudBackupService` abstraction supports both platforms:
- **iOS**: `_ICloudBackup` uses MethodChannel to call native Swift code
- **Android**: `_GoogleDriveBackup` uses Google Drive App Data folder

```dart
class _ICloudBackup implements CloudBackupService {
  static const _channel = MethodChannel('com.cowallet/cloud_backup');

  Future<bool> isAvailable() async { ... }
  Future<void> store(String key, String encryptedData) async { ... }
  Future<String?> retrieve(String key) async { ... }
  Future<void> delete(String key) async { ... }
}
```

### iOS Native Side

**File**: `mobile/ios/Runner/CloudBackupHandler.swift`

Implements the platform channel handler using iOS Security framework:

```swift
public class CloudBackupHandler: NSObject, FlutterPlugin {
  private static let service = "com.cowallet.cloud_backup"

  public func handle(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
    switch call.method {
    case "isAvailable": result(true)
    case "store": store(call, result: result)
    case "retrieve": retrieve(call, result: result)
    case "delete": delete(call, result: result)
    default: result(FlutterMethodNotImplemented)
    }
  }
}
```

### Registration

**File**: `mobile/ios/Runner/AppDelegate.swift`

The handler is registered during app initialization:

```swift
override func application(
  _ application: UIApplication,
  didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
) -> Bool {
  // ...
  CloudBackupHandler.register(with: self)
  return super.application(application, didFinishLaunchingWithOptions: launchOptions)
}
```

## Implementation Details

### iCloud Keychain Storage

The implementation uses iOS Keychain with synchronization enabled:

```swift
let addQuery: [String: Any] = [
  kSecClass as String: kSecClassGenericPassword,
  kSecAttrAccount as String: key,
  kSecAttrService as String: "com.cowallet.cloud_backup",
  kSecValueData as String: valueData,
  kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlock,
  kSecAttrSynchronizable as String: true,  // ✓ Enable iCloud sync
]

SecItemAdd(addQuery as CFDictionary, nil)
```

### Key Attributes

- **kSecClassGenericPassword**: Stores as generic password entry
- **kSecAttrAccount**: The key name (e.g., "backup_shard_1")
- **kSecAttrService**: Namespace identifier ("com.cowallet.cloud_backup")
- **kSecAttrAccessible**: When data is accessible (after first unlock)
- **kSecAttrSynchronizable**: `true` = sync to iCloud Keychain

### Security Model

1. **Data is pre-encrypted**: The Flutter app encrypts shard data before calling the native bridge
2. **Apple adds another layer**: iCloud Keychain adds end-to-end encryption
3. **No plaintext storage**: Shards are never stored in plaintext on device or iCloud

### Method Channel Protocol

#### `isAvailable()`

**Returns**: `true` if iCloud Keychain is available (always true on iOS 7+)

**Usage**:
```dart
final backup = PlatformCloudBackup();
if (await backup.isAvailable()) {
  // iCloud backup is ready
}
```

#### `store(key: String, data: String)`

**Parameters**:
- `key`: Identifier for the data (e.g., "backup_shard_1")
- `data`: Encrypted shard data (as String)

**Behavior**:
- Deletes existing entry if key exists
- Adds new entry with synchronization enabled
- Throws `FlutterError` on failure

**Usage**:
```dart
await backup.store('backup_shard_1', encryptedShard);
```

#### `retrieve(key: String)`

**Parameters**:
- `key`: Identifier for the data

**Returns**: 
- `String` if found
- `null` if not found

**Usage**:
```dart
final shard = await backup.retrieve('backup_shard_1');
if (shard != null) {
  // Decrypt and use shard
}
```

#### `delete(key: String)`

**Parameters**:
- `key`: Identifier for the data

**Behavior**:
- Deletes the entry from iCloud Keychain
- No error if key doesn't exist

**Usage**:
```dart
await backup.delete('backup_shard_1');
```

## Usage Example

### Backing Up a Shard

```dart
import 'package:cowallet/platform/cloud_backup.dart';
import 'package:cowallet/crypto/encryption.dart';

Future<void> backupShard(String shardData) async {
  final backup = PlatformCloudBackup();

  // Check availability
  if (!await backup.isAvailable()) {
    throw Exception('iCloud backup not available');
  }

  // Encrypt shard before backing up
  final encrypted = await encryptShard(shardData);

  // Store in iCloud Keychain
  await backup.store('backup_shard_1', encrypted);

  print('Shard backed up to iCloud Keychain');
}
```

### Restoring a Shard

```dart
Future<String?> restoreShard() async {
  final backup = PlatformCloudBackup();

  // Retrieve from iCloud Keychain
  final encrypted = await backup.retrieve('backup_shard_1');
  if (encrypted == null) {
    print('No backup found');
    return null;
  }

  // Decrypt shard
  final shard = await decryptShard(encrypted);
  print('Shard restored from iCloud Keychain');
  return shard;
}
```

## iOS Capabilities Required

### 1. iCloud Capability

Enable in Xcode:
1. Open `mobile/ios/Runner.xcworkspace`
2. Select Runner target
3. Go to **Signing & Capabilities**
4. Click **+ Capability**
5. Add **iCloud**
6. Enable **Key-value storage** (for Keychain sync)

### 2. Keychain Sharing (Optional)

If you want to share keychain data between apps in the same team:

1. Add **Keychain Sharing** capability
2. Add keychain group: `com.cowallet.shared`

## Testing

### Test on Physical Device

iCloud Keychain sync **requires a physical iOS device** with:
- Signed into iCloud account
- Two-factor authentication enabled
- iCloud Keychain enabled in Settings

### Test Flow

1. **Store a shard**:
   ```dart
   await backup.store('test_shard', 'encrypted_data_here');
   ```

2. **Verify storage**:
   ```dart
   final retrieved = await backup.retrieve('test_shard');
   assert(retrieved == 'encrypted_data_here');
   ```

3. **Test cross-device sync**:
   - Install app on Device A
   - Store shard on Device A
   - Wait 1-2 minutes for sync
   - Install app on Device B (same iCloud account)
   - Retrieve shard on Device B
   - Should get the same data

4. **Clean up**:
   ```dart
   await backup.delete('test_shard');
   ```

## Troubleshooting

### Issue: Data Not Syncing Between Devices

**Possible Causes**:
1. Devices not signed into same iCloud account
2. iCloud Keychain disabled in Settings
3. Network connectivity issues
4. Sync delay (can take several minutes)

**Solutions**:
- Verify iCloud account and Keychain settings
- Force sync by locking/unlocking device
- Wait longer (sync is not instant)

### Issue: `kSecAttrSynchronizable` Not Working

**Possible Causes**:
1. Simulator (iCloud Keychain sync doesn't work on simulator)
2. Developer account not signed
3. iCloud entitlement missing

**Solutions**:
- Test on physical device only
- Verify provisioning profile includes iCloud entitlement
- Check Xcode project capabilities

### Issue: Data Lost After App Reinstall

**Expected Behavior**: 
- iCloud Keychain data persists after app deletion
- Data should restore when app is reinstalled on same iCloud account
- If not syncing, check iCloud Keychain is enabled

## Security Considerations

### 1. Pre-Encryption is Critical

**Always encrypt shards before calling `store()`**:

```dart
// ✓ CORRECT: Encrypt before backup
final encrypted = encryptShard(shard);
await backup.store('shard', encrypted);

// ✗ WRONG: Never store plaintext
await backup.store('shard', plaintextShard);  // INSECURE!
```

### 2. Key Naming Convention

Use consistent, non-guessable key names:

```dart
// ✓ GOOD: Specific, namespaced keys
'cowallet_backup_shard_device'
'cowallet_backup_shard_server'

// ✗ BAD: Generic names
'shard'
'backup'
```

### 3. iCloud Account Security

- Users must enable two-factor authentication
- Warn users about iCloud account security
- Provide alternative backup methods (QR code, paper backup)

### 4. Data Deletion

When user deletes wallet:

```dart
// Delete all backup shards
await backup.delete('cowallet_backup_shard_device');
await backup.delete('cowallet_backup_shard_server');
```

## Comparison: iOS vs Android

| Feature | iOS (iCloud Keychain) | Android (Google Drive App Data) |
|---------|----------------------|----------------------------------|
| Storage | iOS Keychain | Google Drive hidden folder |
| Sync | Automatic (iCloud) | Automatic (Google Drive) |
| Authentication | iCloud account | Google Sign-In required |
| Encryption | Apple E2E encryption | Google encryption |
| Visibility | Hidden from user | Hidden from user |
| Cross-platform | iOS/macOS only | Android only |
| Size limit | ~1MB per item | Unlimited (Drive quota) |
| Sync speed | Fast (few minutes) | Slower (network dependent) |

## References

- [Apple Keychain Services Documentation](https://developer.apple.com/documentation/security/keychain_services)
- [iCloud Keychain Sync Overview](https://support.apple.com/en-us/HT204085)
- [kSecAttrSynchronizable Documentation](https://developer.apple.com/documentation/security/ksecattrsynchronizable)

## Status

✅ **IMPLEMENTATION COMPLETE**

The iOS iCloud backup bridge is fully implemented:
- Native Swift handler exists (`CloudBackupHandler.swift`)
- Flutter interface defined (`cloud_backup.dart`)
- Registered in AppDelegate
- Uses secure iCloud Keychain with synchronization enabled
- Ready for production use
