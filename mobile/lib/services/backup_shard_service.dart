import 'dart:convert';
import 'dart:io';

import 'package:convert/convert.dart';
import 'package:path_provider/path_provider.dart';

import '../platform/cloud_backup.dart';

/// Manages the backup shard (Party 2) for wallet recovery.
///
/// Strategy:
/// 1. If iCloud Keychain / Google Cloud Backup is available → store there
/// 2. Otherwise → generate an encrypted file for user to save manually
class BackupShardService {
  final CloudBackupService _cloud;
  static const _backupKey = 'cowallet_backup_shard';

  BackupShardService(this._cloud);

  /// Store the backup shard. Returns the backup method used.
  /// If cloud is unavailable, returns a file path for the user to save.
  Future<BackupResult> storeBackupShard(List<int> shardBytes, {required bool useCloud}) async {
    final shardHex = hex.encode(shardBytes);
    final payload = _buildBackupPayload(shardHex);

    if (useCloud) {
      if (!await _cloud.isAvailable()) {
        throw BackupException(BackupError.cloudUnavailable);
      }
      try {
        await _cloud.store(_backupKey, payload);
      } catch (_) {
        throw BackupException(BackupError.cloudStoreFailed);
      }
      return BackupResult(method: BackupMethod.cloud);
    }

    try {
      final filePath = await _writeBackupFile(payload);
      return BackupResult(method: BackupMethod.file, filePath: filePath);
    } catch (_) {
      throw BackupException(BackupError.fileWriteFailed);
    }
  }

  /// Retrieve the backup shard from cloud storage.
  Future<List<int>?> retrieveFromCloud() async {
    if (!await _cloud.isAvailable()) return null;

    final payload = await _cloud.retrieve(_backupKey);
    if (payload == null) return null;

    return _parseBackupPayload(payload);
  }

  /// Parse a backup file (user provides file content).
  List<int>? parseBackupFile(String fileContent) {
    return _parseBackupPayload(fileContent);
  }

  /// Check if a cloud backup exists.
  Future<bool> hasCloudBackup() async {
    if (!await _cloud.isAvailable()) return false;
    final data = await _cloud.retrieve(_backupKey);
    return data != null;
  }

  /// Delete the backup shard from cloud.
  Future<void> deleteBackup() async {
    if (await _cloud.isAvailable()) {
      await _cloud.delete(_backupKey);
    }
  }

  String _buildBackupPayload(String shardHex) {
    final data = {
      'version': 1,
      'type': 'cowallet_backup_shard',
      'shard': shardHex,
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
    final file = File('${dir.path}/cowallet_backup_$timestamp.json');
    await file.writeAsString(payload);
    return file.path;
  }
}

enum BackupMethod { cloud, file }

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
