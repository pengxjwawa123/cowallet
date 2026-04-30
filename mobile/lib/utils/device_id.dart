import 'dart:math';
import 'secure_storage.dart';

/// 设备ID生成工具
/// 生成唯一的设备标识符并持久化存储
class DeviceIdGenerator {
  static const _chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789';
  static final _random = Random.secure();

  /// 生成随机设备ID
  static String generate() {
    return List.generate(16, (i) => _chars[_random.nextInt(_chars.length)])
        .join();
  }

  /// 获取设备ID，如果不存在则生成新的
  static Future<String> getOrGenerate() async {
    String? existing = await SecureStorage.getDeviceId();
    if (existing != null && existing.isNotEmpty) {
      return existing;
    }
    String newId = generate();
    await SecureStorage.saveDeviceId(newId);
    return newId;
  }
}
