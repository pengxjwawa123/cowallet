// FFI wrapper for communicating with Rust MPC backend

import 'dart:typed_data';
import 'frb_generated/api.dart' as frb;
import 'frb_generated/frb_generated.dart';

/// Wrapper class for MPC FFI operations
class MpcBridge {
  /// Initialize the Rust FFI bridge. Must be called once at app startup.
  static Future<void> init() async {
    await RustLib.init();
  }

  /// Generate a new wallet (local 2-of-3 MPC key)
  static Future<WalletInfo> generateWallet() async {
    try {
      final ffiInfo = await frb.generateWallet();
      return WalletInfo(
        address: ffiInfo.address,
        publicKey: ffiInfo.publicKey,
      );
    } catch (e) {
      throw MpcException('Failed to generate wallet: $e');
    }
  }

  /// Check if wallet is already initialized
  static Future<bool> hasWallet() async {
    try {
      return await frb.hasWallet();
    } catch (e) {
      throw MpcException('Failed to check wallet: $e');
    }
  }

  /// Get key shard status
  static Future<KeyStatus> getKeyStatus() async {
    try {
      final ffiStatus = await frb.getKeyStatus();
      return KeyStatus(
        hasDeviceShard: ffiStatus.hasDeviceShard,
        hasServerShard: ffiStatus.hasServerShard,
        hasBackupShard: ffiStatus.hasBackupShard,
        address: ffiStatus.address,
      );
    } catch (e) {
      throw MpcException('Failed to get key status: $e');
    }
  }

  /// Clear wallet from memory (destructive)
  static Future<void> clearWallet() async {
    try {
      return await frb.clearWallet();
    } catch (e) {
      throw MpcException('Failed to clear wallet: $e');
    }
  }

  /// ===== DKG Protocol =====

  /// Initialize a new DKG session
  static Future<String> dkgSessionNew(int partyIndex) async {
    try {
      final session = await frb.dkgSessionNew(partyIndex: partyIndex);
      return session.sessionId;
    } catch (e) {
      throw MpcException('Failed to create DKG session: $e');
    }
  }

  /// Generate Round 1 message
  static Future<String> dkgGenerateRound1(String sessionId) async {
    try {
      final result = await frb.dkgGenerateRound1(sessionId: sessionId);
      return result.messageJson;
    } catch (e) {
      throw MpcException('DKG Round 1 generation failed: $e');
    }
  }

  /// Process Round 1 messages from all parties
  static Future<void> dkgProcessRound1(String sessionId, List<String> messagesJson) async {
    try {
      return await frb.dkgProcessRound1(sessionId: sessionId, messagesJson: messagesJson);
    } catch (e) {
      throw MpcException('DKG Round 1 processing failed: $e');
    }
  }

  /// Generate Round 2 messages
  static Future<List<String>> dkgGenerateRound2(String sessionId) async {
    try {
      return await frb.dkgGenerateRound2(sessionId: sessionId);
    } catch (e) {
      throw MpcException('DKG Round 2 generation failed: $e');
    }
  }

  /// Process Round 2 messages
  static Future<void> dkgProcessRound2(String sessionId, List<String> messagesJson) async {
    try {
      return await frb.dkgProcessRound2(sessionId: sessionId, messagesJson: messagesJson);
    } catch (e) {
      throw MpcException('DKG Round 2 processing failed: $e');
    }
  }

  /// Finalize DKG and extract key share
  static Future<WalletInfo> dkgFinalize(String sessionId) async {
    try {
      final result = await frb.dkgFinalize(sessionId: sessionId);
      return WalletInfo(
        address: result.address,
        publicKey: result.publicKey,
      );
    } catch (e) {
      throw MpcException('DKG finalization failed: $e');
    }
  }

  /// Derive the backup shard (Party 2) from DKG session.
  /// Must be called after dkgFinalize. Returns the raw 32-byte secret share.
  static Future<List<int>> dkgDeriveBackupShare(String sessionId, {int backupPartyIndex = 2}) async {
    try {
      return await frb.dkgDeriveBackupShare(sessionId: sessionId, backupPartyIndex: backupPartyIndex);
    } catch (e) {
      throw MpcException('Failed to derive backup share: $e');
    }
  }

  /// ===== Distributed Signing (2-party ECDSA, no key reconstruction) =====

  /// Step 1: Generate Round 1 (R_0 = k_0*G) for the distributed signing protocol.
  /// Returns the payload to send to the server and the session ID.
  static Future<SignRound1Result> signGenerateRound1(List<int> msgHash) async {
    if (msgHash.length != 32) {
      throw MpcException('Message hash must be exactly 32 bytes');
    }
    try {
      final result = await frb.signGenerateRound1(msgHash: Uint8List.fromList(msgHash));
      return SignRound1Result(
        sessionId: result.sessionId,
        payload: result.payload,
        msgHash: result.msgHash,
      );
    } catch (e) {
      throw MpcException('Sign round 1 generation failed: $e');
    }
  }

  /// Step 2: Process server's R_1 and generate DeviceContribution (c_0, k_0_inv).
  /// Returns the Round 2 payload to send to server.
  static Future<List<int>> signProcessRound1AndGenerateRound2(
    String sessionId,
    List<int> serverRound1Payload,
  ) async {
    try {
      return await frb.signProcessRound1AndGenerateRound2(
        sessionId: sessionId,
        serverRound1Payload: Uint8List.fromList(serverRound1Payload),
      );
    } catch (e) {
      throw MpcException('Sign round 1 processing failed: $e');
    }
  }

  /// Step 3: Process server's signature (s) and return final 65-byte signature.
  static Future<List<int>> signProcessRound2(
    String sessionId,
    List<int> serverRound2Payload,
  ) async {
    try {
      final result = await frb.signProcessRound2(
        sessionId: sessionId,
        serverRound2Payload: Uint8List.fromList(serverRound2Payload),
      );
      return result.signature;
    } catch (e) {
      throw MpcException('Sign round 2 processing failed: $e');
    }
  }

  /// Legacy: Sign locally (for testing only — reconstructs full key!)
  static Future<List<int>> signHash(List<int> msgHash) async {
    if (msgHash.length != 32) {
      throw MpcException('Message hash must be exactly 32 bytes');
    }
    try {
      return await frb.signHash(msgHash: Uint8List.fromList(msgHash));
    } catch (e) {
      throw MpcException('Signing failed: $e');
    }
  }

  /// ===== Reshare Protocol (Proactive Key Refresh) =====

  /// Initialize a reshare session with the current device shard.
  static Future<String> reshareSessionNew(int partyIndex) async {
    try {
      final session = await frb.reshareSessionNew(partyIndex: partyIndex);
      return session.sessionId;
    } catch (e) {
      throw MpcException('Failed to create reshare session: $e');
    }
  }

  /// Generate reshare Round 1 messages (new VSS polynomial evaluations).
  static Future<List<String>> reshareGenerateRound1(String sessionId) async {
    try {
      final result = await frb.reshareGenerateRound1(sessionId: sessionId);
      return result.messagesJson;
    } catch (e) {
      throw MpcException('Reshare round 1 generation failed: $e');
    }
  }

  /// Process reshare Round 1 messages from other parties.
  static Future<void> reshareProcessRound1(
    String sessionId,
    List<String> messagesJson,
  ) async {
    try {
      return await frb.reshareProcessRound1(sessionId: sessionId, messagesJson: messagesJson);
    } catch (e) {
      throw MpcException('Reshare round 1 processing failed: $e');
    }
  }

  /// Finalize reshare: replaces old shard with new shard in memory.
  static Future<WalletInfo> reshareFinalize(String sessionId) async {
    try {
      final result = await frb.reshareFinalize(sessionId: sessionId);
      return WalletInfo(
        address: result.address,
        publicKey: result.publicKey,
      );
    } catch (e) {
      throw MpcException('Reshare finalization failed: $e');
    }
  }

  /// ===== Presign Protocol (Pre-computed signing material) =====

  /// Generate presign Round 1 (ephemeral k, R_0 = k_0*G) without a message hash.
  static Future<PresignRound1Result> presignGenerateRound1() async {
    try {
      final result = await frb.presignGenerateRound1();
      return PresignRound1Result(
        sessionId: result.sessionId,
        payload: result.payload,
      );
    } catch (e) {
      throw MpcException('Presign round 1 generation failed: $e');
    }
  }

  /// Process server's presign Round 1 and generate Round 2.
  static Future<List<int>> presignProcessRound1AndGenerateRound2(
    String sessionId,
    List<int> serverRound1Payload,
  ) async {
    try {
      return await frb.presignProcessRound1AndGenerateRound2(
        sessionId: sessionId,
        serverRound1Payload: Uint8List.fromList(serverRound1Payload),
      );
    } catch (e) {
      throw MpcException('Presign round 1 processing failed: $e');
    }
  }

  /// Finalize presign and extract opaque presignature data.
  static Future<List<int>> presignFinalize(String sessionId) async {
    try {
      final result = await frb.presignFinalize(sessionId: sessionId);
      return result.presigData;
    } catch (e) {
      throw MpcException('Presign finalization failed: $e');
    }
  }

  /// ===== Recovery Protocol (Restore device shard using backup + server) =====

  /// Import and validate the backup shard for recovery.
  /// The backup shard is stored temporarily until recovery is complete.
  static Future<void> recoveryImportBackupShard(List<int> backupBytes) async {
    try {
      return await frb.recoveryImportBackupShard(backupBytes: Uint8List.fromList(backupBytes));
    } catch (e) {
      throw MpcException('Failed to import backup shard: $e');
    }
  }

  /// Reconstruct the device shard (Party 0) using backup + server contributions.
  /// Returns the recovered wallet info with address and public key.
  static Future<WalletInfo> recoveryReconstructDeviceShard({
    required String sessionId,
    required List<String> serverMessagesJson,
    required List<int> publicKey,
  }) async {
    try {
      final result = await frb.recoveryReconstructDeviceShard(
        sessionId: sessionId,
        serverMessagesJson: serverMessagesJson,
        publicKey: Uint8List.fromList(publicKey),
      );
      return WalletInfo(
        address: result.address,
        publicKey: result.publicKey,
      );
    } catch (e) {
      throw MpcException('Recovery reconstruction failed: $e');
    }
  }

  /// Clear the temporary backup shard from recovery state.
  static Future<void> recoveryClearBackupShard() async {
    try {
      return await frb.recoveryClearBackupShard();
    } catch (e) {
      throw MpcException('Failed to clear recovery backup: $e');
    }
  }

  /// Check if a backup shard has been imported for recovery.
  static Future<bool> recoveryHasBackupShard() async {
    try {
      return await frb.recoveryHasBackupShard();
    } catch (e) {
      throw MpcException('Failed to check recovery backup: $e');
    }
  }

  /// ===== Backup Shard Combination =====

  /// Combine device and server backup share contributions into the final backup shard.
  /// Performs modular addition: backup_shard = device_share + server_share (mod secp256k1_order).
  /// Both inputs must be exactly 32 bytes.
  static Future<List<int>> combineBackupShares({
    required List<int> deviceShare,
    required List<int> serverShare,
  }) async {
    if (deviceShare.length != 32) {
      throw MpcException('deviceShare must be 32 bytes, got ${deviceShare.length}');
    }
    if (serverShare.length != 32) {
      throw MpcException('serverShare must be 32 bytes, got ${serverShare.length}');
    }

    try {
      return await frb.combineBackupShares(
        deviceShare: Uint8List.fromList(deviceShare),
        serverShare: Uint8List.fromList(serverShare),
      );
    } catch (e) {
      throw MpcException('Failed to combine backup shares: $e');
    }
  }
}

/// Result from distributed sign Round 1
class SignRound1Result {
  final String sessionId;
  final List<int> payload;
  final List<int> msgHash;

  SignRound1Result({
    required this.sessionId,
    required this.payload,
    required this.msgHash,
  });
}

/// Model classes for Dart

class WalletInfo {
  final String address;
  final List<int> publicKey;

  WalletInfo({
    required this.address,
    required this.publicKey,
  });

  @override
  String toString() => 'WalletInfo(address: $address, publicKeyLen: ${publicKey.length})';
}

class KeyStatus {
  final bool hasDeviceShard;
  final bool hasServerShard;
  final bool hasBackupShard;
  final String address;

  KeyStatus({
    required this.hasDeviceShard,
    required this.hasServerShard,
    required this.hasBackupShard,
    required this.address,
  });

  bool get isComplete => hasDeviceShard && hasServerShard && hasBackupShard;

  @override
  String toString() =>
      'KeyStatus(device: $hasDeviceShard, server: $hasServerShard, backup: $hasBackupShard, address: $address)';
}

class DkgSession {
  final String sessionId;

  DkgSession({required this.sessionId});

  @override
  String toString() => 'DkgSession(id: $sessionId)';
}

/// Result from presign Round 1
class PresignRound1Result {
  final String sessionId;
  final List<int> payload;

  PresignRound1Result({
    required this.sessionId,
    required this.payload,
  });
}

/// Exception type for MPC operations
class MpcException implements Exception {
  final String message;

  MpcException(this.message);

  @override
  String toString() => 'MpcException: $message';
}
