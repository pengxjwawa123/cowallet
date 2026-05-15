import 'dart:async';
import '../api/mpc_api.dart';
import '../bridge/mpc_bridge.dart';
import '../network/mpc_websocket.dart';
import 'mpc_session_store.dart';
import 'mpc_wallet_service.dart';

/// Manages MPC session recovery and resumption after interruptions.
/// Wraps MpcWalletService with automatic session persistence and recovery.
class MpcSessionManager {
  final MpcWalletService _mpcService;

  MpcSessionManager(this._mpcService);

  /// Check if there's a session that can be resumed.
  Future<bool> canResume() async {
    return await MpcSessionStore.hasActiveSession();
  }

  /// Attempt to resume an interrupted session.
  /// Returns session info if resumable, null if session is stale/failed.
  Future<MpcSessionState?> checkResumableSession() async {
    final session = await MpcSessionStore.loadSession();
    if (session == null) return null;

    // Check backend session status
    try {
      final result = await MpcApi.getSession(session.remoteSessionId);
      if (!result.isSuccess || result.data == null) {
        // Session not found on backend, clear local state
        await MpcSessionStore.clearSession();
        return null;
      }

      final status = result.data!['status'] as String;
      final backendRound = result.data!['current_round'] as int;

      // Only resume if session is still active
      if (status == 'active') {
        // Update local round if backend is ahead
        if (backendRound > session.currentRound) {
          await MpcSessionStore.updateCurrentRound(backendRound);
          return session.copyWith(currentRound: backendRound);
        }
        return session;
      } else {
        // Session failed/completed on backend, clear local state
        await MpcSessionStore.clearSession();
        return null;
      }
    } catch (e) {
      print('[MpcSessionManager] Error checking session: $e');
      // Network error, keep local state for now
      return session;
    }
  }

  /// Run DKG with automatic session persistence and recovery.
  Future<WalletInfo> runDkgWithRecovery({String? walletId}) async {
    // Check for existing session
    final existing = await checkResumableSession();
    if (existing != null && existing.sessionType == 'keygen') {
      print('[MpcSessionManager] Attempting to resume DKG session ${existing.remoteSessionId}');
      try {
        final result = await _resumeDkg(existing);
        await MpcSessionStore.clearSession();
        return result;
      } catch (e) {
        print('[MpcSessionManager] Resume failed: $e, starting fresh');
        await MpcSessionStore.clearSession();
      }
    }

    // Start new session with persistence
    return await _runDkgWithPersistence(walletId: walletId);
  }

  /// Run Sign with automatic session persistence and recovery.
  Future<List<int>> runSignWithRecovery(List<int> msgHash, {String? walletId}) async {
    // Check for existing session
    final existing = await checkResumableSession();
    if (existing != null && existing.sessionType == 'sign') {
      print('[MpcSessionManager] Attempting to resume Sign session ${existing.remoteSessionId}');
      try {
        final result = await _resumeSign(existing, msgHash);
        await MpcSessionStore.clearSession();
        return result;
      } catch (e) {
        print('[MpcSessionManager] Resume failed: $e, starting fresh');
        await MpcSessionStore.clearSession();
      }
    }

    // Start new session with persistence
    return await _runSignWithPersistence(msgHash, walletId: walletId);
  }

  /// Run Reshare with automatic session persistence and recovery.
  Future<WalletInfo> runReshareWithRecovery({String? walletId}) async {
    // Check for existing session
    final existing = await checkResumableSession();
    if (existing != null && existing.sessionType == 'reshare') {
      print('[MpcSessionManager] Attempting to resume Reshare session ${existing.remoteSessionId}');
      try {
        final result = await _resumeReshare(existing);
        await MpcSessionStore.clearSession();
        return result;
      } catch (e) {
        print('[MpcSessionManager] Resume failed: $e, starting fresh');
        await MpcSessionStore.clearSession();
      }
    }

    // Start new session with persistence
    return await _runReshareWithPersistence(walletId: walletId);
  }

  // ==================== DKG with Persistence ====================

  Future<WalletInfo> _runDkgWithPersistence({String? walletId}) async {
    // Delegate to original service implementation with wrapped error handling
    return await _mpcService.runDkg(walletId: walletId);
  }

  Future<WalletInfo> _resumeDkg(MpcSessionState session) async {
    // For DKG, we can't truly resume the Rust crypto state.
    // If the session was interrupted, the best we can do is restart.
    // However, we can check if the backend session completed while we were offline.
    final result = await MpcApi.getSession(session.remoteSessionId);
    if (!result.isSuccess || result.data == null) {
      throw MpcSessionInterruptedException('DKG session not found on backend');
    }

    final status = result.data!['status'] as String;
    if (status == 'completed') {
      // Backend completed the session, but we need the wallet info.
      // This is a rare edge case - for now, throw to start fresh.
      throw MpcSessionInterruptedException('DKG session completed on backend but local state lost');
    }

    throw MpcSessionInterruptedException('DKG cannot be resumed, restart required');
  }

  // ==================== Sign with Persistence ====================

  Future<List<int>> _runSignWithPersistence(List<int> msgHash, {String? walletId}) async {
    // Delegate to original service implementation
    return await _mpcService.runSign(msgHash, walletId: walletId);
  }

  Future<List<int>> _resumeSign(MpcSessionState session, List<int> msgHash) async {
    // Sign protocol cannot be resumed from interruption because the Rust state is ephemeral.
    // Throw to restart the signing session.
    throw MpcSessionInterruptedException('Sign session cannot be resumed, restart required');
  }

  // ==================== Reshare with Persistence ====================

  Future<WalletInfo> _runReshareWithPersistence({String? walletId}) async {
    // Delegate to original service implementation
    return await _mpcService.runReshare(walletId: walletId);
  }

  Future<WalletInfo> _resumeReshare(MpcSessionState session) async {
    // Reshare protocol cannot be resumed from interruption.
    throw MpcSessionInterruptedException('Reshare session cannot be resumed, restart required');
  }

  /// Fetch missed messages from backend (for message replay).
  Future<List<MpcMessage>> _fetchMissedMessages({
    required String sessionId,
    required int party,
    required int afterId,
  }) async {
    final result = await MpcApi.receiveMessages(
      sessionId,
      party: party,
      afterId: afterId,
    );

    if (!result.isSuccess || result.data == null) {
      return [];
    }

    return result.data!.map((raw) {
      final m = Map<String, dynamic>.from(raw as Map);
      return MpcMessage(
        fromParty: m['from_party'] as int,
        toParty: m['to_party'] as int,
        round: m['round'] as int,
        payload: (m['payload'] as List<dynamic>).cast<int>(),
      );
    }).toList();
  }

  /// Exponential backoff retry for transient network errors.
  Future<T> _retryWithBackoff<T>(
    Future<T> Function() operation, {
    int maxAttempts = 3,
    Duration initialDelay = const Duration(seconds: 1),
  }) async {
    int attempt = 0;
    Duration delay = initialDelay;

    while (true) {
      try {
        return await operation();
      } catch (e) {
        attempt++;
        if (attempt >= maxAttempts) {
          rethrow;
        }

        print('[MpcSessionManager] Retry $attempt/$maxAttempts after ${delay.inSeconds}s: $e');
        await Future.delayed(delay);
        delay *= 2; // Exponential backoff
      }
    }
  }
}
