// FFI wrapper for communicating with Rust MPC backend

import 'dart:convert';
import 'ffi.dart';

/// Wrapper class for MPC FFI operations
class MpcBridge {
  /// Generate a new wallet (local 2-of-3 MPC key)
  static Future<WalletInfo> generateWallet() async {
    try {
      final ffiInfo = await api.generateWallet();
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
      return await api.hasWallet();
    } catch (e) {
      throw MpcException('Failed to check wallet: $e');
    }
  }

  /// Get key shard status
  static Future<KeyStatus> getKeyStatus() async {
    try {
      final ffiStatus = await api.getKeyStatus();
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
      return await api.clearWallet();
    } catch (e) {
      throw MpcException('Failed to clear wallet: $e');
    }
  }

  /// ===== DKG Protocol =====

  /// Initialize a new DKG session
  static Future<String> dkgSessionNew(int partyIndex) async {
    try {
      final session = await api.dkgSessionNew(partyIndex);
      return session.sessionId;
    } catch (e) {
      throw MpcException('Failed to create DKG session: $e');
    }
  }

  /// Generate Round 1 message
  static Future<String> dkgGenerateRound1(String sessionId) async {
    try {
      final result = await api.dkgGenerateRound1(sessionId);
      return result.messageJson;
    } catch (e) {
      throw MpcException('DKG Round 1 generation failed: $e');
    }
  }

  /// Process Round 1 messages from all parties
  static Future<void> dkgProcessRound1(String sessionId, List<String> messagesJson) async {
    try {
      return await api.dkgProcessRound1(sessionId, messagesJson);
    } catch (e) {
      throw MpcException('DKG Round 1 processing failed: $e');
    }
  }

  /// Generate Round 2 messages
  static Future<List<String>> dkgGenerateRound2(String sessionId) async {
    try {
      return await api.dkgGenerateRound2(sessionId);
    } catch (e) {
      throw MpcException('DKG Round 2 generation failed: $e');
    }
  }

  /// Process Round 2 messages
  static Future<void> dkgProcessRound2(String sessionId, List<String> messagesJson) async {
    try {
      return await api.dkgProcessRound2(sessionId, messagesJson);
    } catch (e) {
      throw MpcException('DKG Round 2 processing failed: $e');
    }
  }

  /// Finalize DKG and extract key share
  static Future<WalletInfo> dkgFinalize(String sessionId) async {
    try {
      final result = await api.dkgFinalize(sessionId);
      return WalletInfo(
        address: result.address,
        publicKey: result.publicKey,
      );
    } catch (e) {
      throw MpcException('DKG finalization failed: $e');
    }
  }

  /// ===== Signing =====

  /// Sign a message hash (32 bytes)
  static Future<List<int>> signHash(List<int> msgHash) async {
    if (msgHash.length != 32) {
      throw MpcException('Message hash must be exactly 32 bytes');
    }
    try {
      return await api.signHash(msgHash);
    } catch (e) {
      throw MpcException('Signing failed: $e');
    }
  }
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

/// Exception type for MPC operations
class MpcException implements Exception {
  final String message;

  MpcException(this.message);

  @override
  String toString() => 'MpcException: $message';
}
