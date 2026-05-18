import 'dart:convert';
import 'dart:io';

import 'package:convert/convert.dart';
import 'package:path_provider/path_provider.dart';

import '../bridge/mpc_bridge.dart';
import '../platform/cloud_backup.dart';
import '../utils/secure_storage.dart';


/// Manages the backup shard (Party 2) for wallet recovery.
///
/// Strategy:
/// 1. If iCloud Keychain / Google Cloud Backup is available → store there
/// 2. Otherwise → generate an encrypted file for user to save manually
class BackupShardService {
  final CloudBackupService _cloud;
  static const _backupKeyPrefix = 'cowallet_backup_shard_';
  static const _methodKeyPrefix = 'backup_shard_method_';
  String? _walletAddress;

  BackupShardService(this._cloud);

  void setWalletAddress(String address) {
    _walletAddress = address.toLowerCase();
  }

  Future<String> _getBackupKey() async => '$_backupKeyPrefix${await _getAddressSuffix()}';
  Future<String> _getMethodKey() async => '$_methodKeyPrefix${await _getAddressSuffix()}';

  Future<String> _getAddressSuffix() async {
    if (_walletAddress != null && _walletAddress!.isNotEmpty) {
      return _walletAddress!.substring(0, 10);
    }
    final addr = await SecureStorage.get('mpc_address');
    if (addr != null && addr.length >= 10) {
      _walletAddress = addr.toLowerCase();
      return _walletAddress!.substring(0, 10);
    }
    return 'unknown';
  }

  /// Store the backup shard. Returns the backup method used.
  /// If cloud is unavailable, returns a file path for the user to save.
  Future<BackupResult> storeBackupShard(List<int> shardBytes, {required bool useCloud}) async {
    final shardHex = hex.encode(shardBytes);

    if (useCloud) {
      final payload = _buildBackupPayload(shardHex);
      if (!await _cloud.isAvailable()) {
        throw BackupException(BackupError.cloudUnavailable);
      }
      try {
        await _cloud.store(await _getBackupKey(), payload);
      } catch (_) {
        throw BackupException(BackupError.cloudStoreFailed);
      }
      await SecureStorage.save(await _getMethodKey(), 'cloud');
      return BackupResult(method: BackupMethod.cloud);
    }

    try {
      final payload = _buildBackupPayload(shardHex);
      final filePath = await _writeBackupFile(payload);
      await SecureStorage.save(await _getMethodKey(), 'file');
      return BackupResult(method: BackupMethod.file, filePath: filePath);
    } catch (_) {
      throw BackupException(BackupError.fileWriteFailed);
    }
  }

  /// Retrieve the backup shard from cloud storage.
  Future<List<int>?> retrieveFromCloud() async {
    if (!await _cloud.isAvailable()) return null;

    final payload = await _cloud.retrieve(await _getBackupKey());
    if (payload == null) return null;

    return _parseBackupPayload(payload);
  }

  /// Parse a backup file (user provides file content).
  List<int>? parseBackupFile(String fileContent) {
    return _parseBackupPayload(fileContent);
  }

  /// Get the stored backup method (cloud, file, or encrypted_file).
  Future<BackupMethod?> getBackupMethod() async {
    final method = await SecureStorage.get(await _getMethodKey());
    if (method == 'cloud') return BackupMethod.cloud;
    if (method == 'file') return BackupMethod.file;
    if (method == 'encrypted_file') return BackupMethod.encryptedFile;
    return null;
  }

  /// Check if a cloud backup exists.
  Future<bool> hasCloudBackup() async {
    if (!await _cloud.isAvailable()) return false;
    final data = await _cloud.retrieve(await _getBackupKey());
    return data != null;
  }

  /// Delete the backup shard from cloud.
  Future<void> deleteBackup() async {
    if (await _cloud.isAvailable()) {
      await _cloud.delete(await _getBackupKey());
    }
  }

  // ---------------------------------------------------------------------------
  // Password-Encrypted Export/Import (via Rust FFI)
  // ---------------------------------------------------------------------------

  /// Export the backup shard as a password-encrypted base64 string.
  /// Uses Argon2id KDF + AES-256-GCM in Rust for maximum security.
  /// The resulting string is safe for QR codes, file storage, or clipboard.
  ///
  /// Password must be at least 8 characters.
  Future<String> exportEncrypted(String password) async {
    return MpcBridge.exportBackupShard(password: password);
  }

  /// Import a backup shard from a password-encrypted base64 string.
  /// Decrypts, validates (must be valid secp256k1 scalar), and stores in memory.
  ///
  /// Returns true on success. Throws on wrong password or corrupted data.
  Future<bool> importEncrypted(String encryptedData, String password) async {
    return MpcBridge.importBackupShard(
      encryptedData: encryptedData,
      password: password,
    );
  }

  /// Export the encrypted backup to a file and return the file path.
  /// Combines password-encrypted export with file storage.
  Future<String> exportEncryptedToFile(String password) async {
    final encrypted = await exportEncrypted(password);
    final dir = await _getExportDirectory();
    final timestamp = DateTime.now().millisecondsSinceEpoch;
    final suffix = _walletAddress != null ? '_${_walletAddress!.substring(0, 10)}' : '';
    final file = File('${dir.path}/cowallet_backup${suffix}_$timestamp.enc');
    await file.writeAsString(encrypted);
    await SecureStorage.save(await _getMethodKey(), 'encrypted_file');
    await SecureStorage.save('backup_exported_at', DateTime.now().toIso8601String());
    return file.path;
  }

  /// Check if the backup has been exported (any method).
  Future<bool> hasExportedBackup() async {
    final method = await getBackupMethod();
    return method != null;
  }

  Future<Directory> _getExportDirectory() async {
    if (Platform.isAndroid) {
      final dir = Directory('/storage/emulated/0/Download');
      if (await dir.exists()) return dir;
    }
    return getApplicationDocumentsDirectory();
  }

  String _buildBackupPayload(String shardHex) {
    final data = {
      'version': 2,
      'type': 'cowallet_backup_shard',
      'shard': shardHex,
      if (_walletAddress != null) 'wallet_address': _walletAddress,
      'created_at': DateTime.now().toIso8601String(),
    };
    return jsonEncode(data);
  }

  List<int>? _parseBackupPayload(String payload) {
    try {
      final data = jsonDecode(payload) as Map<String, dynamic>;
      if (data['type'] != 'cowallet_backup_shard') return null;
      final shardHex = data['shard'] as String?;
      if (shardHex == null) return null;
      return hex.decode(shardHex);
    } catch (_) {
      return null;
    }
  }

  Future<String> _writeBackupFile(String payload) async {
    Directory dir;
    if (Platform.isAndroid) {
      dir = Directory('/storage/emulated/0/Download');
      if (!await dir.exists()) {
        dir = await getApplicationDocumentsDirectory();
      }
    } else {
      dir = await getApplicationDocumentsDirectory();
    }
    final timestamp = DateTime.now().millisecondsSinceEpoch;
    final suffix = _walletAddress != null ? '_${_walletAddress!.substring(0, 10)}' : '';
    final file = File('${dir.path}/cowallet_backup${suffix}_$timestamp.json');
    await file.writeAsString(payload);
    return file.path;
  }
}

enum BackupMethod { cloud, file, encryptedFile }

class BackupResult {
  final BackupMethod method;
  final String? filePath;

  BackupResult({required this.method, this.filePath});
}

enum BackupError { cloudUnavailable, cloudStoreFailed, fileWriteFailed, shardNotAvailable }

class BackupException implements Exception {
  final BackupError error;
  BackupException(this.error);

  @override
  String toString() => error.name;
}
