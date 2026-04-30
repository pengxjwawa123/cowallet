// Platform channel for native SE/Keystore integration

import 'package:flutter/services.dart';

/// Channel for iOS Secure Enclave operations
class IosSecureEnclaveChannel {
  static const platform = MethodChannel('com.cowallet.mpc/se');
  static const secureStorage = MethodChannel('com.cowallet.mpc/storage');

  /// Generate a new key in Secure Enclave
  /// Returns: Map with 'publicKey' (base64), 'keyId' (String)
  static Future<Map<String, dynamic>> generateKey(String keyId) async {
    try {
      final result = await platform.invokeMethod<Map<dynamic, dynamic>>(
        'generateKey',
        {'keyId': keyId},
      );
      return Map<String, dynamic>.from(result ?? {});
    } catch (e) {
      throw SeException('Failed to generate key: $e');
    }
  }

  /// Get the public key for a key ID
  /// Returns: base64-encoded public key (33 bytes, compressed)
  static Future<String> getPublicKey(String keyId) async {
    try {
      final result = await platform.invokeMethod<String>(
        'getPublicKey',
        {'keyId': keyId},
      );
      return result ?? '';
    } catch (e) {
      throw SeException('Failed to get public key: $e');
    }
  }

  /// Sign a message with biometric authentication
  /// Returns: base64-encoded signature (64 or 65 bytes)
  static Future<String> signWithBiometric(
    String keyId,
    String message,
    String reason,
  ) async {
    try {
      final result = await platform.invokeMethod<String>(
        'signWithBiometric',
        {
          'keyId': keyId,
          'message': message,
          'reason': reason,
        },
      );
      return result ?? '';
    } catch (e) {
      throw SeException('Biometric authentication failed: $e');
    }
  }

  /// Check if Secure Enclave is available on this device
  static Future<bool> isAvailable() async {
    try {
      final result = await platform.invokeMethod<bool>('isAvailable');
      return result ?? false;
    } catch (e) {
      return false;
    }
  }

  /// Store encrypted data in secure storage
  static Future<void> storeSecret(String key, String value) async {
    try {
      await secureStorage.invokeMethod(
        'storeSecret',
        {'key': key, 'value': value},
      );
    } catch (e) {
      throw SeException('Failed to store secret: $e');
    }
  }

  /// Retrieve encrypted data from secure storage
  static Future<String?> getSecret(String key) async {
    try {
      final result = await secureStorage.invokeMethod<String>(
        'getSecret',
        {'key': key},
      );
      return result;
    } catch (e) {
      throw SeException('Failed to retrieve secret: $e');
    }
  }

  /// Delete encrypted data from secure storage
  static Future<void> deleteSecret(String key) async {
    try {
      await secureStorage.invokeMethod(
        'deleteSecret',
        {'key': key},
      );
    } catch (e) {
      throw SeException('Failed to delete secret: $e');
    }
  }
}

/// Exception for Secure Enclave operations
class SeException implements Exception {
  final String message;
  SeException(this.message);

  @override
  String toString() => 'SeException: $message';
}
