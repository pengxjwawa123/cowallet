import '../bridge/mpc_bridge.dart';
import '../network/dio_client.dart';
import '../utils/secure_storage.dart';
import 'backup_shard_service.dart';

/// Wallet recovery service for restoring wallets on a new device.
///
/// Recovery protocol:
/// 1. User authenticates via email + OTP
/// 2. User provides backup shard (from iCloud/Google Cloud or file import)
/// 3. Client + Server perform a special reshare to reconstruct device shard (Party 0)
/// 4. The backup (Party 2) + server (Party 1) generate new device shard without changing public key
class RecoveryService {
  final BackupShardService _backupService;

  String? _recoverySessionId;
  String? _accessToken;
  String? _userId;

  RecoveryService(this._backupService);

  /// Step 1: Initiate recovery by requesting OTP to user's email.
  /// Returns the recovery session ID.
  Future<RecoveryInitiateResult> initiateRecovery(String email) async {
    final result = await DioClient.post<Map<String, dynamic>>(
      '/auth/recovery/initiate',
      data: {'email': email},
    );

    if (!result.isSuccess || result.data == null) {
      throw RecoveryException(
        'Failed to initiate recovery: ${result.errorMessage}',
      );
    }

    final data = result.data!;
    _recoverySessionId = data['recovery_session_id'] as String;

    return RecoveryInitiateResult(
      recoverySessionId: _recoverySessionId!,
      message: data['message'] as String? ?? 'OTP sent to your email',
    );
  }

  /// Step 2: Verify OTP and authenticate.
  /// Returns access token and server's contribution for device shard reconstruction.
  Future<RecoveryVerifyResult> verifyOtp({
    required String otp,
    required String deviceId,
  }) async {
    if (_recoverySessionId == null) {
      throw RecoveryException(
        'Recovery not initiated. Call initiateRecovery first.',
      );
    }

    final result = await DioClient.post<Map<String, dynamic>>(
      '/auth/recovery/verify',
      data: {
        'recovery_session_id': _recoverySessionId,
        'otp': otp,
        'device_id': deviceId,
      },
    );

    if (!result.isSuccess || result.data == null) {
      final code = result.errorCode;
      if (code == 401) {
        throw RecoveryException('验证码错误，请重新输入');
      } else if (code == 410) {
        throw RecoveryException('验证码已过期，请重新发起恢复');
      } else if (code == 429) {
        throw RecoveryException('尝试次数过多，请重新发起恢复');
      } else if (code == 409) {
        throw RecoveryException('该恢复会话已使用，请重新发起');
      }
      throw RecoveryException(
        'OTP verification failed: ${result.errorMessage}',
      );
    }

    final data = result.data!;
    _accessToken = data['token'] as String;
    _userId = data['user_id'] as String;

    // Store tokens for future API calls
    await SecureStorage.save('access_token', _accessToken!);
    await SecureStorage.save('refresh_token', data['refresh_token'] as String);
    await SecureStorage.save('user_id', _userId!);

    return RecoveryVerifyResult(
      accessToken: _accessToken!,
      publicKeyHex: data['public_key_hex'] as String,
      serverReshareMessagesJson:
          (data['server_reshare_messages_json'] as List<dynamic>)
              .cast<String>(),
      serverCommitmentHex: data['server_commitment_hex'] as String,
    );
  }

  /// Step 3: Import backup shard from cloud or file.
  Future<void> importBackupShard({
    BackupShardSource source = BackupShardSource.cloud,
    String? fileContent,
  }) async {
    List<int>? backupBytes;

    switch (source) {
      case BackupShardSource.cloud:
        backupBytes = await _backupService.retrieveFromCloud();
        if (backupBytes == null) {
          throw RecoveryException('No backup shard found in cloud storage');
        }
        break;

      case BackupShardSource.file:
        if (fileContent == null) {
          throw RecoveryException('File content is required for file import');
        }
        backupBytes = _backupService.parseBackupFile(fileContent);
        if (backupBytes == null) {
          throw RecoveryException('Invalid backup file format');
        }
        break;
    }

    // Import to Rust FFI layer
    await MpcBridge.recoveryImportBackupShard(backupBytes);
  }

  /// Step 4: Execute recovery protocol.
  /// Reconstructs the device shard using backup + server contributions.
  Future<RecoveryResult> executeRecovery({
    required String publicKeyHex,
    required List<String> serverReshareMessagesJson,
    required String serverCommitmentHex,
  }) async {
    // Check that backup shard is imported
    final hasBackup = await MpcBridge.recoveryHasBackupShard();
    if (!hasBackup) {
      throw RecoveryException(
        'Backup shard not imported. Call importBackupShard first.',
      );
    }

    // Convert public key hex to bytes
    final publicKeyBytes = _hexToBytes(publicKeyHex);
    final serverCommitmentBytes = _hexToBytes(serverCommitmentHex);

    // Reconstruct device shard via FFI (verifies backup shard via Feldman commitment first)
    final walletInfo = await MpcBridge.recoveryReconstructDeviceShard(
      sessionId: _recoverySessionId ?? 'recovery',
      serverMessagesJson: serverReshareMessagesJson,
      publicKey: publicKeyBytes,
      serverCommitment: serverCommitmentBytes,
    );

    // Store recovered wallet address
    await SecureStorage.save('mpc_address', walletInfo.address);
    await SecureStorage.save('recovery_completed', 'true');

    return RecoveryResult(
      address: walletInfo.address,
      publicKey: walletInfo.publicKey,
      userId: _userId ?? '',
    );
  }

  /// Full recovery flow: initiate → verify OTP → import backup → execute.
  Future<RecoveryResult> recoverWallet({
    required String email,
    required String otp,
    required String deviceId,
    BackupShardSource backupSource = BackupShardSource.cloud,
    String? backupFileContent,
  }) async {
    // Step 1: Initiate
    await initiateRecovery(email);

    // Step 2: Verify OTP
    final verifyResult = await verifyOtp(otp: otp, deviceId: deviceId);

    // Step 3: Import backup shard
    await importBackupShard(
      source: backupSource,
      fileContent: backupFileContent,
    );

    // Step 4: Execute recovery
    return await executeRecovery(
      publicKeyHex: verifyResult.publicKeyHex,
      serverReshareMessagesJson: verifyResult.serverReshareMessagesJson,
      serverCommitmentHex: verifyResult.serverCommitmentHex,
    );
  }

  /// Check if cloud backup is available for this device.
  Future<bool> hasCloudBackup() async {
    return await _backupService.hasCloudBackup();
  }

  /// Clear recovery state (for cancellation or cleanup).
  Future<void> clearRecoveryState() async {
    await MpcBridge.recoveryClearBackupShard();
    _recoverySessionId = null;
    _accessToken = null;
    _userId = null;
  }

  List<int> _hexToBytes(String hex) {
    // Remove 0x prefix if present
    final cleanHex = hex.startsWith('0x') ? hex.substring(2) : hex;
    final bytes = <int>[];
    for (int i = 0; i < cleanHex.length; i += 2) {
      bytes.add(int.parse(cleanHex.substring(i, i + 2), radix: 16));
    }
    return bytes;
  }
}

enum BackupShardSource {
  cloud,
  file,
}

class RecoveryInitiateResult {
  final String recoverySessionId;
  final String message;

  RecoveryInitiateResult({
    required this.recoverySessionId,
    required this.message,
  });
}

class RecoveryVerifyResult {
  final String accessToken;
  final String publicKeyHex;
  final List<String> serverReshareMessagesJson;
  final String serverCommitmentHex;

  RecoveryVerifyResult({
    required this.accessToken,
    required this.publicKeyHex,
    required this.serverReshareMessagesJson,
    required this.serverCommitmentHex,
  });
}

class RecoveryResult {
  final String address;
  final List<int> publicKey;
  final String userId;

  RecoveryResult({
    required this.address,
    required this.publicKey,
    required this.userId,
  });
}

class RecoveryException implements Exception {
  final String message;

  RecoveryException(this.message);

  @override
  String toString() => 'RecoveryException: $message';
}
