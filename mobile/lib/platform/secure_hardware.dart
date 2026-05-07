import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'ios_se_channel.dart';
import 'android_strongbox_channel.dart';

/// Hardware security level information
class HardwareSecurityInfo {
  final bool isAvailable;
  final String securityLevel; // 'SecureEnclave', 'StrongBox', 'TEE', or 'Software'
  final bool biometricRequired;

  HardwareSecurityInfo({
    required this.isAvailable,
    required this.securityLevel,
    required this.biometricRequired,
  });
}

/// Unified interface for hardware-backed secure storage
/// Uses iOS Secure Enclave or Android StrongBox/TEE
class SecureHardware {
  SecureHardware._();

  static final SecureHardware _instance = SecureHardware._();
  factory SecureHardware() => _instance;

  /// Check if hardware security is available on this device
  static Future<bool> isAvailable() async {
    if (Platform.isIOS) {
      return await IosSecureEnclaveChannel.isAvailable();
    } else if (Platform.isAndroid) {
      return await AndroidStrongBoxChannel.isAvailable();
    }
    return false;
  }

  /// Get detailed hardware security information
  static Future<HardwareSecurityInfo> getInfo() async {
    if (Platform.isIOS) {
      final available = await IosSecureEnclaveChannel.isAvailable();
      return HardwareSecurityInfo(
        isAvailable: available,
        securityLevel: available ? 'SecureEnclave' : 'Software',
        biometricRequired: available,
      );
    } else if (Platform.isAndroid) {
      final available = await AndroidStrongBoxChannel.isAvailable();
      return HardwareSecurityInfo(
        isAvailable: available,
        securityLevel: available ? 'StrongBox' : 'TEE',
        biometricRequired: available,
      );
    }
    return HardwareSecurityInfo(
      isAvailable: false,
      securityLevel: 'Software',
      biometricRequired: false,
    );
  }

  /// Store encrypted device shard (key never leaves hardware)
  /// The shard bytes are encrypted using a hardware-backed key and stored securely
  static Future<void> storeDeviceShard(Uint8List shardBytes) async {
    if (Platform.isIOS) {
      await IosSecureEnclaveChannel.storeEncryptedShard(shardBytes);
    } else if (Platform.isAndroid) {
      await AndroidStrongBoxChannel.storeEncryptedShard(shardBytes);
    } else {
      throw UnsupportedError('Platform not supported');
    }
  }

  /// Retrieve decrypted device shard (requires biometric auth on supported platforms)
  /// Returns null if no shard is stored
  static Future<Uint8List?> loadDeviceShard() async {
    if (Platform.isIOS) {
      final result = await IosSecureEnclaveChannel.loadEncryptedShard();
      return result != null ? Uint8List.fromList(result) : null;
    } else if (Platform.isAndroid) {
      final result = await AndroidStrongBoxChannel.loadEncryptedShard();
      return result != null ? Uint8List.fromList(result) : null;
    } else {
      throw UnsupportedError('Platform not supported');
    }
  }

  /// Delete device shard from hardware store
  static Future<void> deleteDeviceShard() async {
    if (Platform.isIOS) {
      await IosSecureEnclaveChannel.deleteSecret('device-shard-encrypted');
    } else if (Platform.isAndroid) {
      await AndroidStrongBoxChannel.deleteSecret('device-shard-encrypted');
    } else {
      throw UnsupportedError('Platform not supported');
    }
  }

  /// Sign a message hash with the hardware-backed device key
  /// Requires biometric authentication
  static Future<Uint8List> signHash(
    Uint8List messageHash,
    String reason,
  ) async {
    if (messageHash.length != 32) {
      throw ArgumentError('Message hash must be 32 bytes');
    }

    if (Platform.isIOS) {
      // Get the device key ID
      final keyId = await IosSecureEnclaveChannel.getSecret('device-key-id');
      if (keyId == null) {
        throw StateError('Device key not initialized');
      }

      final signatureBase64 = await IosSecureEnclaveChannel.signWithBiometric(
        keyId,
        _bytesToBase64(messageHash),
        reason,
      );
      return _base64ToBytes(signatureBase64);
    } else if (Platform.isAndroid) {
      final keyId = await AndroidStrongBoxChannel.getSecret('device-key-id');
      if (keyId == null) {
        throw StateError('Device key not initialized');
      }

      final signatureBase64 = await AndroidStrongBoxChannel.signWithBiometric(
        keyId,
        _bytesToBase64(messageHash),
        reason,
      );
      return _base64ToBytes(signatureBase64);
    } else {
      throw UnsupportedError('Platform not supported');
    }
  }

  /// Initialize hardware security (generates encryption key if needed)
  /// Should be called during onboarding after biometric setup
  static Future<void> initialize(String deviceId) async {
    final keyId = 'device-shard-key-$deviceId';

    if (Platform.isIOS) {
      // Check if key already exists
      final existingKeyId = await IosSecureEnclaveChannel.getSecret('device-key-id');
      if (existingKeyId != null) {
        return; // Already initialized
      }

      // Generate new key in Secure Enclave
      await IosSecureEnclaveChannel.generateKey(keyId);
      await IosSecureEnclaveChannel.storeSecret('device-key-id', keyId);
    } else if (Platform.isAndroid) {
      final existingKeyId = await AndroidStrongBoxChannel.getSecret('device-key-id');
      if (existingKeyId != null) {
        return; // Already initialized
      }

      // Generate new key in StrongBox/Keystore
      await AndroidStrongBoxChannel.generateKey(keyId);
      await AndroidStrongBoxChannel.storeSecret('device-key-id', keyId);
    } else {
      throw UnsupportedError('Platform not supported');
    }
  }

  /// Clear all hardware-backed data (for wallet reset)
  static Future<void> clear() async {
    if (Platform.isIOS) {
      await IosSecureEnclaveChannel.deleteSecret('device-key-id');
      await IosSecureEnclaveChannel.deleteSecret('device-shard-encrypted');
    } else if (Platform.isAndroid) {
      await AndroidStrongBoxChannel.deleteSecret('device-key-id');
      await AndroidStrongBoxChannel.deleteSecret('device-shard-encrypted');
    }
  }

  // Helper methods for base64 encoding/decoding
  static String _bytesToBase64(Uint8List bytes) {
    return base64Encode(bytes);
  }

  static Uint8List _base64ToBytes(String base64) {
    return Uint8List.fromList(base64Decode(base64));
  }
}
