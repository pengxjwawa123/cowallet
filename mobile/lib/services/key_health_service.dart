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
  static const verifyExpiryDays = 30;
  final _backupService = BackupShardService(PlatformCloudBackup());
  static const _lastUsedPhonePrefix = 'key_phone_last_used_';
  static const _lastUsedServerPrefix = 'key_server_last_used_';
  static const _lastCheckedBackupPrefix = 'key_backup_last_checked_';

  Future<String> _getWalletSuffix() async {
    final addr = await SecureStorage.get('mpc_address');
    if (addr != null && addr.length >= 10) return addr.toLowerCase().substring(0, 10);
    return 'unknown';
  }

  Future<String> _getBackupCheckedKey() async => '$_lastCheckedBackupPrefix${await _getWalletSuffix()}';

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

      final suffix = await _getWalletSuffix();
      final lastUsedStr = await SecureStorage.get('$_lastUsedPhonePrefix$suffix');
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
      final suffix = await _getWalletSuffix();
      final lastUsedStr = await SecureStorage.get('$_lastUsedServerPrefix$suffix');
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
      final backupCheckedKey = await _getBackupCheckedKey();
      final lastCheckedStr = await SecureStorage.get(backupCheckedKey);
      final lastChecked = lastCheckedStr != null ? DateTime.tryParse(lastCheckedStr) : null;

      final method = await _backupService.getBackupMethod();

      // Local file backup cannot be auto-verified
      if (method == BackupMethod.file) {
        if (lastChecked != null) {
          return KeyHealth(
            status: KeyStatus.ok,
            lastChecked: lastChecked,
          );
        }
        return KeyHealth(
          status: KeyStatus.warning,
          lastChecked: null,
          error: 'file_not_verified',
        );
      }

      // Cloud backup check
      if (lastChecked != null) {
        return KeyHealth(
          status: KeyStatus.ok,
          lastChecked: lastChecked,
        );
      }

      final hasBackup = await _backupService.hasCloudBackup();
      return KeyHealth(
        status: hasBackup ? KeyStatus.warning : KeyStatus.unknown,
        lastChecked: null,
        error: hasBackup ? 'cloud_not_verified' : 'cloud_not_found',
      );
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

  Future<bool> testBackupKey() async {
    try {
      final shardBytes = await _backupService.retrieveFromCloud();
      if (shardBytes == null || shardBytes.length != 32) {
        print('[KeyHealth] cloud backup not available or invalid: ${shardBytes?.length}');
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

      await SecureStorage.save(await _getBackupCheckedKey(), DateTime.now().toIso8601String());
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

      await SecureStorage.save(await _getBackupCheckedKey(), DateTime.now().toIso8601String());
      return true;
    } catch (e) {
      print('[KeyHealth] testBackupKeyWithFile error: $e');
      return false;
    }
  }

  Future<void> recordPhoneKeyUsage() async {
    final suffix = await _getWalletSuffix();
    await SecureStorage.save('$_lastUsedPhonePrefix$suffix', DateTime.now().toIso8601String());
  }

  Future<void> recordServerKeyUsage() async {
    final suffix = await _getWalletSuffix();
    await SecureStorage.save('$_lastUsedServerPrefix$suffix', DateTime.now().toIso8601String());
  }
}
