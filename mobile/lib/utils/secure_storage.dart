import 'package:flutter_secure_storage/flutter_secure_storage.dart';

class SecureStorage {
  static const FlutterSecureStorage _storage = FlutterSecureStorage(
    aOptions: AndroidOptions(encryptedSharedPreferences: true),
    iOptions: IOSOptions(accessibility: KeychainAccessibility.first_unlock),
  );

  // 存储key常量
  static const String keyToken = "user_token";
  static const String keyRefreshToken = "refresh_token";
  static const String keyUserId = "user_id";
  static const String keyWalletAddress = "wallet_address";
  static const String keyMnemonic = "wallet_mnemonic";
  static const String keyDeviceId = "device_id";

  // 存token
  static Future<void> saveToken(String token) async {
    await _storage.write(key: keyToken, value: token);
  }

  // 取token
  static Future<String?> getToken() async {
    return await _storage.read(key: keyToken);
  }

  // 删除token（退出登录时用）
  static Future<void> deleteToken() async {
    await _storage.delete(key: keyToken);
  }

  // 存refresh_token
  static Future<void> saveRefreshToken(String token) async {
    await _storage.write(key: keyRefreshToken, value: token);
  }

  // 取refresh_token
  static Future<String?> getRefreshToken() async {
    return await _storage.read(key: keyRefreshToken);
  }

  // 清除认证相关数据（不影响生物识别、钱包等设置）
  static Future<void> clearAuthData() async {
    await _storage.delete(key: keyToken);
    await _storage.delete(key: keyRefreshToken);
  }

  // 存助记词（加密存储，钱包必备）
  static Future<void> saveMnemonic(String mnemonic) async {
    await _storage.write(key: keyMnemonic, value: mnemonic);
  }

  // 取助记词
  static Future<String?> getMnemonic() async {
    return await _storage.read(key: keyMnemonic);
  }

  // 存设备ID
  static Future<void> saveDeviceId(String deviceId) async {
    await _storage.write(key: keyDeviceId, value: deviceId);
  }

  // 取设备ID
  static Future<String?> getDeviceId() async {
    return await _storage.read(key: keyDeviceId);
  }

  // 存用户ID
  static Future<void> saveUserId(String userId) async {
    await _storage.write(key: keyUserId, value: userId);
  }

  // 取用户ID
  static Future<String?> getUserId() async {
    return await _storage.read(key: keyUserId);
  }

  // 通用存储方法
  static Future<void> save(String key, String value) async {
    await _storage.write(key: key, value: value);
  }

  static Future<String?> get(String key) async {
    return await _storage.read(key: key);
  }

  static Future<void> delete(String key) async {
    await _storage.delete(key: key);
  }

  static Future<void> clearAll() async {
    await _storage.deleteAll();
  }
}
