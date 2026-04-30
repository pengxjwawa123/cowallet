// Android StrongBox Channel Interface
// Communication layer between Dart and Android native code

import 'package:flutter/services.dart';

/// Exception for StrongBox channel errors
class SbException implements Exception {
  final String message;

  SbException(this.message);

  @override
  String toString() => 'SbException: $message';
}

/// Android StrongBox channel interface
class AndroidStrongBoxChannel {
  static const platform = MethodChannel('com.cowallet.mpc/strongbox');
  static const storageChannel = MethodChannel('com.cowallet.mpc/keystore');

  /// Check if StrongBox is available on this device
  static Future<bool> isAvailable() async {
    try {
      final result = await platform.invokeMethod<bool>('isAvailable');
      return result ?? false;
    } catch (e) {
      throw SbException('Failed to check StrongBox availability: $e');
    }
  }

  /// Generate a new key in StrongBox
  /// Returns: {publicKey: base64, keyId: String}
  static Future<Map<String, dynamic>> generateKey(String keyId) async {
    try {
      final result = await platform.invokeMapMethod<String, dynamic>(
        'generateKey',
        {'keyId': keyId},
      );

      if (result == null) {
        throw SbException('generateKey returned null');
      }

      return result;
    } on PlatformException catch (e) {
      throw SbException('generateKey failed: ${e.message}');
    }
  }

  /// Get public key for a key ID
  /// Returns: base64 encoded compressed public key (33 bytes)
  static Future<String> getPublicKey(String keyId) async {
    try {
      final result = await platform.invokeMethod<String>(
        'getPublicKey',
        {'keyId': keyId},
      );

      if (result == null) {
        throw SbException('getPublicKey returned null');
      }

      return result;
    } on PlatformException catch (e) {
      throw SbException('getPublicKey failed: ${e.message}');
    }
  }

  /// Sign a message hash with biometric authentication
  /// hash: base64 encoded 32-byte message hash
  /// reason: user-visible reason for biometric prompt
  /// Returns: base64 encoded signature
  static Future<String> signWithBiometric(
    String keyId,
    String hash,
    String reason,
  ) async {
    try {
      final result = await platform.invokeMethod<String>(
        'signWithBiometric',
        {
          'keyId': keyId,
          'hash': hash,
          'reason': reason,
        },
      );

      if (result == null) {
        throw SbException('signWithBiometric returned null');
      }

      return result;
    } on PlatformException catch (e) {
      throw SbException('signWithBiometric failed: ${e.message}');
    }
  }

  // MARK: - Secure Storage

  /// Store encrypted data in Android Keystore
  static Future<void> storeSecret(String key, String value) async {
    try {
      await storageChannel.invokeMethod<void>(
        'storeSecret',
        {
          'key': key,
          'value': value,
        },
      );
    } on PlatformException catch (e) {
      throw SbException('storeSecret failed: ${e.message}');
    }
  }

  /// Retrieve encrypted data from Android Keystore
  /// Returns: null if key not found
  static Future<String?> getSecret(String key) async {
    try {
      final result = await storageChannel.invokeMethod<String>(
        'getSecret',
        {'key': key},
      );

      return result;
    } on PlatformException catch (e) {
      throw SbException('getSecret failed: ${e.message}');
    }
  }

  /// Delete encrypted data from Android Keystore
  static Future<void> deleteSecret(String key) async {
    try {
      await storageChannel.invokeMethod<void>(
        'deleteSecret',
        {'key': key},
      );
    } on PlatformException catch (e) {
      throw SbException('deleteSecret failed: ${e.message}');
    }
  }
}
