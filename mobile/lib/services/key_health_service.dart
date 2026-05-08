import '../platform/se_manager.dart';
import '../platform/sb_manager.dart';
import '../platform/cloud_backup.dart';
import '../api/mpc_api.dart';
import '../utils/secure_storage.dart';

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

  /// Check key 3: backup (cloud or file)
  Future<KeyHealth> checkBackupKey() async {
    try {
      final lastCheckedStr = await SecureStorage.get(_lastCheckedBackupKey);
      final lastChecked = lastCheckedStr != null ? DateTime.tryParse(lastCheckedStr) : null;

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

  /// Test key 3: attempt to retrieve and verify the backup shard
  Future<bool> testBackupKey() async {
    try {
      final cloudAvailable = await _cloudBackup.isAvailable();
      if (!cloudAvailable) return false;

      final data = await _cloudBackup.retrieve('cowallet_backup_shard');
      if (data == null || data.isEmpty) return false;

      // Mark as verified
      await SecureStorage.save(_lastCheckedBackupKey, DateTime.now().toIso8601String());
      return true;
    } catch (_) {
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
