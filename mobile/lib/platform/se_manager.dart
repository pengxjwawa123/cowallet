// Secure Enclave Manager for managing key generation, storage, and signing

import 'dart:convert';

import 'ios_se_channel.dart';

/// Manager for Secure Enclave operations on iOS
class SecureEnclaveManager {
  static final SecureEnclaveManager _instance = SecureEnclaveManager._internal();

  factory SecureEnclaveManager() {
    return _instance;
  }

  SecureEnclaveManager._internal();

  /// Check if SE is available on this device
  Future<bool> isAvailable() async {
    return await IosSecureEnclaveChannel.isAvailable();
  }

  /// Initialize SE for wallet (generates device shard key)
  /// Returns: wallet address derived from public key
  Future<String> initializeWallet(String deviceId) async {
    try {
      final keyId = 'device-shard-$deviceId';

      // Generate key in SE
      final keyInfo = await IosSecureEnclaveChannel.generateKey(keyId);
      final publicKeyBase64 = keyInfo['publicKey'] as String;

      // Store key ID for later reference
      await IosSecureEnclaveChannel.storeSecret(
        'device-shard-key-id',
        keyId,
      );

      // Derive address from public key (simplified)
      // In real implementation, this would use proper address derivation
      final publicKeyBytes = base64Decode(publicKeyBase64);
      final address = _deriveAddressFromPublicKey(publicKeyBytes);

      return address;
    } catch (e) {
      throw SeException('Failed to initialize wallet: $e');
    }
  }

  /// Get the stored device shard key ID
  Future<String?> getDeviceShardKeyId() async {
    return await IosSecureEnclaveChannel.getSecret('device-shard-key-id');
  }

  /// Get public key for device shard
  Future<List<int>> getDeviceShardPublicKey() async {
    try {
      final keyId = await getDeviceShardKeyId();
      if (keyId == null) {
        throw SeException('Device shard not initialized');
      }

      final publicKeyBase64 = await IosSecureEnclaveChannel.getPublicKey(keyId);
      return base64Decode(publicKeyBase64);
    } catch (e) {
      throw SeException('Failed to get device shard public key: $e');
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
        throw SeException('Device shard not initialized');
      }

      final signature = await IosSecureEnclaveChannel.signWithBiometric(
        keyId,
        hash,
        reason,
      );

      return signature;
    } catch (e) {
      throw SeException('Signing failed: $e');
    }
  }

  /// Store encrypted device shard data
  Future<void> storeDeviceShard(String encryptedData) async {
    try {
      await IosSecureEnclaveChannel.storeSecret(
        'device-shard-data',
        encryptedData,
      );
    } catch (e) {
      throw SeException('Failed to store device shard: $e');
    }
  }

  /// Retrieve encrypted device shard data
  Future<String?> getDeviceShard() async {
    try {
      return await IosSecureEnclaveChannel.getSecret('device-shard-data');
    } catch (e) {
      throw SeException('Failed to retrieve device shard: $e');
    }
  }

  /// Delete all SE-related data (wallet reset)
  Future<void> clearWallet() async {
    try {
      final keyId = await getDeviceShardKeyId();
      if (keyId != null) {
        // Note: We cannot delete SE keys from Dart directly
        // This would require a native method
      }

      await IosSecureEnclaveChannel.deleteSecret('device-shard-key-id');
      await IosSecureEnclaveChannel.deleteSecret('device-shard-data');
    } catch (e) {
      throw SeException('Failed to clear wallet: $e');
    }
  }

  // MARK: - Helper Functions

  /// Derive Ethereum address from public key (compressed 33 bytes)
  String _deriveAddressFromPublicKey(List<int> publicKeyBytes) {
    if (publicKeyBytes.length != 33) {
      throw SeException('Invalid public key length');
    }

    // Decompress public key (33 bytes -> 65 bytes)
    final uncompressed = _decompressPublicKey(publicKeyBytes);

    // Keccak256(uncompressed_pubkey)[12:32] for address
    // This is a simplified version; real implementation would use actual Keccak256
    final addressBytes = uncompressed.sublist(12, 32);
    final address = '0x${addressBytes.map((b) => b.toRadixString(16).padLeft(2, '0')).join()}';

    return address;
  }

  /// Decompress a compressed public key (33 bytes -> 65 bytes)
  List<int> _decompressPublicKey(List<int> compressed) {
    if (compressed.length != 33 || (compressed[0] != 0x02 && compressed[0] != 0x03)) {
      throw SeException('Invalid compressed public key format');
    }

    // For now, return a placeholder
    // Real implementation would use proper EC math
    final uncompressed = <int>[0x04];
    uncompressed.addAll(compressed.sublist(1));
    uncompressed.addAll(List<int>.filled(32, 0)); // Placeholder Y coordinate

    return uncompressed;
  }
}

/// Exception for Secure Enclave Manager operations
class SeManagerException implements Exception {
  final String message;

  SeManagerException(this.message);

  @override
  String toString() => 'SeManagerException: $message';
}
