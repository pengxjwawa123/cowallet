import '../platform/se_manager.dart';
import '../platform/sb_manager.dart';
import '../platform/cloud_backup.dart';
import '../platform/secure_hardware.dart';
import '../api/mpc_api.dart';
import '../bridge/mpc_bridge.dart';
import '../utils/secure_storage.dart';
import 'backup_shard_service.dart';

enum KeyStatus { ok, warning, error, unknown }

class KeyHealth {
  final KeyStatus status;
  final DateTime? lastUsed;
  final DateTime? lastChecked;
  final String? error;

  KeyHealth({
    required this.status,
    this.lastUsed,
    this.lastChecked,
    this.error,
  });
}

class KeyHealthService {
  final _cloudBackup = PlatformCloudBackup();
  final _backupService = BackupShardService(PlatformCloudBackup());
  static const _lastUsedPhoneKey = 'key_phone_last_used';
  static const _lastUsedServerKey = 'key_server_last_used';
  static const _lastCheckedBackupKey = 'key_backup_last_checked';

  /// Check key 1: phone (Secure Enclave / StrongBox)
  Future<KeyHealth> checkPhoneKey() async {
    try {
      final se = SecureEnclaveManager();
      final sb = StrongBoxManager();

      bool available = false;
      if (await se.isAvailable()) {
        available = true;
      } else if (await sb.isAvailable()) {
        available = true;
      }

      final lastUsedStr = await SecureStorage.get(_lastUsedPhoneKey);
      final lastUsed = lastUsedStr != null ? DateTime.tryParse(lastUsedStr) : null;

      return KeyHealth(
        status: available ? KeyStatus.ok : KeyStatus.error,
        lastUsed: lastUsed,
        lastChecked: DateTime.now(),
        error: available ? null : 'Hardware security not available',
      );
    } catch (e) {
      return KeyHealth(status: KeyStatus.error, error: e.toString());
    }
  }

  /// Check key 2: server heartbeat
  Future<KeyHealth> checkServerKey() async {
    try {
      final result = await MpcApi.getServerShardStatus();
      final lastUsedStr = await SecureStorage.get(_lastUsedServerKey);
      final lastUsed = lastUsedStr != null ? DateTime.tryParse(lastUsedStr) : null;

      if (result.isSuccess) {
        return KeyHealth(
          status: KeyStatus.ok,
          lastUsed: lastUsed,
          lastChecked: DateTime.now(),
        );
      } else {
        return KeyHealth(
          status: KeyStatus.warning,
          lastUsed: lastUsed,
          lastChecked: DateTime.now(),
          error: result.errorMessage,
        );
      }
    } catch (e) {
      return KeyHealth(
        status: KeyStatus.error,
        lastChecked: DateTime.now(),
        error: e.toString(),
      );
    }
  }

  /// Get the backup method used during setup.
  Future<BackupMethod?> getBackupMethod() async {
    return await _backupService.getBackupMethod();
  }

  /// Check key 3: backup (cloud or file)
  Future<KeyHealth> checkBackupKey() async {
    try {
      final lastCheckedStr = await SecureStorage.get(_lastCheckedBackupKey);
      final lastChecked = lastCheckedStr != null ? DateTime.tryParse(lastCheckedStr) : null;

      final method = await _backupService.getBackupMethod();

      // If backup is stored as local file, we can't auto-check — rely on last verified timestamp
      if (method == BackupMethod.file) {
        if (lastChecked != null) {
          final days = DateTime.now().difference(lastChecked).inDays;
          return KeyHealth(
            status: days > 90 ? KeyStatus.warning : KeyStatus.ok,
            lastChecked: lastChecked,
            error: days > 90 ? 'Local file not verified for $days days' : null,
          );
        }
        return KeyHealth(
          status: KeyStatus.warning,
          lastChecked: null,
          error: 'Local backup file needs verification',
        );
      }

      // Cloud backup check
      final cloudAvailable = await _cloudBackup.isAvailable();
      if (!cloudAvailable) {
        return KeyHealth(
          status: lastChecked != null ? KeyStatus.warning : KeyStatus.unknown,
          lastChecked: lastChecked,
          error: 'Cloud not available for verification',
        );
      }

      final data = await _cloudBackup.retrieve('cowallet_backup_shard');
      if (data != null && data.isNotEmpty) {
        return KeyHealth(
          status: KeyStatus.ok,
          lastChecked: DateTime.now(),
        );
      } else {
        return KeyHealth(
          status: lastChecked != null ? KeyStatus.warning : KeyStatus.unknown,
          lastChecked: lastChecked,
        );
      }
    } catch (e) {
      return KeyHealth(
        status: KeyStatus.error,
        error: e.toString(),
      );
    }
  }

  /// Load device shard from hardware storage and public key from secure storage.
  Future<({List<int> deviceShard, List<int> publicKey})> _loadVerificationData() async {
    final deviceShard = await SecureHardware.loadDeviceShard();
    if (deviceShard == null || deviceShard.length != 32) {
      throw Exception('Device shard not available');
    }

    final pubKeyHex = await SecureStorage.get('mpc_public_key');
    if (pubKeyHex == null || pubKeyHex.isEmpty) {
      throw Exception('Public key not found');
    }

    final publicKey = _hexToBytes(pubKeyHex);
    return (deviceShard: deviceShard.toList(), publicKey: publicKey);
  }

  List<int> _hexToBytes(String hex) {
    final cleanHex = hex.startsWith('0x') ? hex.substring(2) : hex;
    final bytes = <int>[];
    for (int i = 0; i < cleanHex.length; i += 2) {
      bytes.add(int.parse(cleanHex.substring(i, i + 2), radix: 16));
    }
    return bytes;
  }

  /// Test key 3 (cloud): retrieve the backup shard and verify it cryptographically
  /// against the device shard by reconstructing the public key.
  Future<bool> testBackupKey() async {
    try {
      final cloudAvailable = await _cloudBackup.isAvailable();
      if (!cloudAvailable) {
        print('[KeyHealth] cloud not available');
        return false;
      }

      final data = await _cloudBackup.retrieve('cowallet_backup_shard');
      if (data == null || data.isEmpty) {
        print('[KeyHealth] cloud backup data is empty');
        return false;
      }

      final shardBytes = _backupService.parseBackupFile(data);
      if (shardBytes == null || shardBytes.length != 32) {
        print('[KeyHealth] cloud parseBackupFile failed: shardBytes=${shardBytes?.length}');
        return false;
      }
      print('[KeyHealth] cloud parsed backup shard: ${shardBytes.length} bytes');

      final vData = await _loadVerificationData();
      print('[KeyHealth] cloud deviceShard: ${vData.deviceShard.length} bytes, pubKey: ${vData.publicKey.length} bytes');

      final valid = await MpcBridge.verifyBackupShard(
        backupBytes: shardBytes,
        deviceShardBytes: vData.deviceShard,
        expectedPublicKey: vData.publicKey,
      );
      print('[KeyHealth] cloud verifyBackupShard result: $valid');
      if (!valid) return false;

      await SecureStorage.save(_lastCheckedBackupKey, DateTime.now().toIso8601String());
      return true;
    } catch (e) {
      print('[KeyHealth] testBackupKey (cloud) error: $e');
      return false;
    }
  }

  /// Test key 3 (file): validate the user-provided local JSON file by verifying it
  /// cryptographically against the device shard (Lagrange interpolation → public key match).
  Future<bool> testBackupKeyWithFile(String fileContent) async {
    try {
      final shardBytes = _backupService.parseBackupFile(fileContent);
      if (shardBytes == null || shardBytes.length != 32) {
        print('[KeyHealth] parseBackupFile failed: shardBytes=${shardBytes?.length}');
        return false;
      }
      print('[KeyHealth] parsed backup shard: ${shardBytes.length} bytes');

      final vData = await _loadVerificationData();
      print('[KeyHealth] deviceShard: ${vData.deviceShard.length} bytes, pubKey: ${vData.publicKey.length} bytes');

      final valid = await MpcBridge.verifyBackupShard(
        backupBytes: shardBytes,
        deviceShardBytes: vData.deviceShard,
        expectedPublicKey: vData.publicKey,
      );
      print('[KeyHealth] verifyBackupShard result: $valid');
      if (!valid) return false;

      await SecureStorage.save(_lastCheckedBackupKey, DateTime.now().toIso8601String());
      return true;
    } catch (e) {
      print('[KeyHealth] testBackupKeyWithFile error: $e');
      return false;
    }
  }

  /// Record phone key usage (call after signing)
  Future<void> recordPhoneKeyUsage() async {
    await SecureStorage.save(_lastUsedPhoneKey, DateTime.now().toIso8601String());
  }

  /// Record server key usage (call after signing)
  Future<void> recordServerKeyUsage() async {
    await SecureStorage.save(_lastUsedServerKey, DateTime.now().toIso8601String());
  }
}
