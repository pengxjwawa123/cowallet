// Android StrongBox Manager for managing key generation, storage, and signing

import 'dart:convert';

import 'android_strongbox_channel.dart';

/// Manager for Android StrongBox operations
class StrongBoxManager {
  static final StrongBoxManager _instance = StrongBoxManager._internal();

  factory StrongBoxManager() {
    return _instance;
  }

  StrongBoxManager._internal();

  /// Check if StrongBox is available on this device
  Future<bool> isAvailable() async {
    return await AndroidStrongBoxChannel.isAvailable();
  }

  /// Initialize StrongBox for wallet (generates device shard key)
  /// Returns: wallet address derived from public key
  Future<String> initializeWallet(String deviceId) async {
    try {
      final keyId = 'device-shard-$deviceId';

      // Generate key in StrongBox
      final keyInfo = await AndroidStrongBoxChannel.generateKey(keyId);
      final publicKeyBase64 = keyInfo['publicKey'] as String;

      // Store key ID for later reference
      await AndroidStrongBoxChannel.storeSecret(
        'device-shard-key-id',
        keyId,
      );

      // Derive address from public key (simplified)
      // In real implementation, this would use proper address derivation
      final publicKeyBytes = base64Decode(publicKeyBase64);
      final address = _deriveAddressFromPublicKey(publicKeyBytes);

      return address;
    } catch (e) {
      throw SbManagerException('Failed to initialize wallet: $e');
    }
  }

  /// Get the stored device shard key ID
  Future<String?> getDeviceShardKeyId() async {
    return await AndroidStrongBoxChannel.getSecret('device-shard-key-id');
  }

  /// Get public key for device shard
  Future<List<int>> getDeviceShardPublicKey() async {
    try {
      final keyId = await getDeviceShardKeyId();
      if (keyId == null) {
        throw SbManagerException('Device shard not initialized');
      }

      final publicKeyBase64 = await AndroidStrongBoxChannel.getPublicKey(keyId);
      return base64Decode(publicKeyBase64);
    } catch (e) {
      throw SbManagerException('Failed to get device shard public key: $e');
    }
  }

  /// Sign a message hash with biometric authentication
  /// hash: 32-byte message hash (base64 encoded)
  /// Returns: signature (base64 encoded)
  Future<String> signHashWithBiometric(
    String hash,
    String reason,
  ) async {
    try {
      final keyId = await getDeviceShardKeyId();
      if (keyId == null) {
        throw SbManagerException('Device shard not initialized');
      }

      final signature = await AndroidStrongBoxChannel.signWithBiometric(
        keyId,
        hash,
        reason,
      );

      return signature;
    } catch (e) {
      throw SbManagerException('Signing failed: $e');
    }
  }

  /// Store encrypted device shard data
  Future<void> storeDeviceShard(String encryptedData) async {
    try {
      await AndroidStrongBoxChannel.storeSecret(
        'device-shard-data',
        encryptedData,
      );
    } catch (e) {
      throw SbManagerException('Failed to store device shard: $e');
    }
  }

  /// Retrieve encrypted device shard data
  Future<String?> getDeviceShard() async {
    try {
      return await AndroidStrongBoxChannel.getSecret('device-shard-data');
    } catch (e) {
      throw SbManagerException('Failed to retrieve device shard: $e');
    }
  }

  /// Delete all StrongBox-related data (wallet reset)
  Future<void> clearWallet() async {
    try {
      final keyId = await getDeviceShardKeyId();
      if (keyId != null) {
        // Note: We cannot delete StrongBox keys from Dart directly
        // This would require a native method or factory reset
      }

      await AndroidStrongBoxChannel.deleteSecret('device-shard-key-id');
      await AndroidStrongBoxChannel.deleteSecret('device-shard-data');
    } catch (e) {
      throw SbManagerException('Failed to clear wallet: $e');
    }
  }

  // MARK: - Helper Functions

  /// Derive Ethereum address from public key (RSA, convert to secp256k1 format)
  String _deriveAddressFromPublicKey(List<int> publicKeyBytes) {
    if (publicKeyBytes.isEmpty) {
      throw SbManagerException('Invalid public key');
    }

    // For RSA keys, extract the modulus as an approximation
    // Real implementation would properly derive Ethereum address
    // This is a placeholder for demonstration
    final addressBytes = publicKeyBytes.sublist(publicKeyBytes.length - 20);
    final address = '0x${addressBytes.map((b) => b.toRadixString(16).padLeft(2, '0')).join()}';

    return address;
  }
}

/// Exception for StrongBox Manager operations
class SbManagerException implements Exception {
  final String message;

  SbManagerException(this.message);

  @override
  String toString() => 'SbManagerException: $message';
}
