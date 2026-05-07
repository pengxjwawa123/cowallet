import 'package:flutter/services.dart';

/// Cloud backup service for storing encrypted shard data.
/// Uses iCloud Keychain on iOS and Google Encrypted Backup on Android.
abstract class CloudBackupService {
  Future<bool> isAvailable();
  Future<void> store(String key, String encryptedData);
  Future<String?> retrieve(String key);
  Future<void> delete(String key);
}

class PlatformCloudBackup implements CloudBackupService {
  static const _channel = MethodChannel('com.cowallet/cloud_backup');

  @override
  Future<bool> isAvailable() async {
    try {
      final result = await _channel.invokeMethod<bool>('isAvailable');
      return result ?? false;
    } on PlatformException {
      return false;
    }
  }

  @override
  Future<void> store(String key, String encryptedData) async {
    await _channel.invokeMethod('store', {
      'key': key,
      'data': encryptedData,
    });
  }

  @override
  Future<String?> retrieve(String key) async {
    try {
      return await _channel.invokeMethod<String>('retrieve', {'key': key});
    } on PlatformException {
      return null;
    }
  }

  @override
  Future<void> delete(String key) async {
    await _channel.invokeMethod('delete', {'key': key});
  }
}
