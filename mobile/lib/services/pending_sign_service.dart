import 'dart:async';
import '../api/mpc_api.dart';
import 'mpc_session_store.dart';

/// Represents a pending MPC session that was interrupted and may be resumable.
class PendingSession {
  final String sessionId;
  final String sessionType;
  final String status;
  final int currentRound;
  final String? walletId;
  final DateTime createdAt;
  final DateTime? lastActivity;

  PendingSession({
    required this.sessionId,
    required this.sessionType,
    required this.status,
    required this.currentRound,
    this.walletId,
    required this.createdAt,
    this.lastActivity,
  });

  factory PendingSession.fromJson(Map<String, dynamic> json) {
    return PendingSession(
      sessionId: json['session_id'] as String,
      sessionType: json['session_type'] as String,
      status: json['status'] as String,
      currentRound: json['current_round'] as int,
      walletId: json['wallet_id'] as String?,
      createdAt: DateTime.parse(json['created_at'] as String),
      lastActivity: json['last_activity'] != null
          ? DateTime.parse(json['last_activity'] as String)
          : null,
    );
  }

  /// Whether this session is a sign session that might be resumable.
  bool get isResumableSign =>
      sessionType == 'sign' && currentRound >= 1 && currentRound < 3;

  /// Time remaining before expiry (sessions expire 5 min after creation).
  Duration get timeRemaining {
    final expiresAt = createdAt.add(const Duration(minutes: 5));
    final remaining = expiresAt.difference(DateTime.now());
    return remaining.isNegative ? Duration.zero : remaining;
  }

  /// Whether this session has expired.
  bool get isExpired => timeRemaining == Duration.zero;
}

/// Service that checks for interrupted MPC sessions on app start
/// and provides recovery options to the user.
class PendingSignService {
  final StreamController<PendingSession?> _pendingController =
      StreamController<PendingSession?>.broadcast();

  /// Stream that emits when a pending session is detected.
  /// UI listens to this to show recovery prompt.
  Stream<PendingSession?> get onPendingSession => _pendingController.stream;

  PendingSession? _currentPending;

  /// The currently detected pending session (if any).
  PendingSession? get currentPending => _currentPending;

  /// Check for interrupted sessions on both local store and backend.
  /// Called on app start or resume.
  Future<PendingSession?> checkForPendingSessions() async {
    // First check local session store
    final localSession = await MpcSessionStore.loadSession();
    if (localSession != null && localSession.sessionType == 'sign') {
      // Verify it's still valid on the backend
      try {
        final result = await MpcApi.getSession(localSession.remoteSessionId);
        if (result.isSuccess && result.data != null) {
          final status = result.data!['status'] as String;
          if (status == 'active' || status == 'interrupted') {
            final pending = PendingSession(
              sessionId: localSession.remoteSessionId,
              sessionType: localSession.sessionType,
              status: status,
              currentRound: localSession.currentRound,
              walletId: localSession.metadata?['wallet_id'] as String?,
              createdAt: localSession.createdAt,
            );

            if (!pending.isExpired) {
              _currentPending = pending;
              _pendingController.add(pending);
              return pending;
            }
          }
        }
      } catch (e) {
        print('[PendingSignService] Error checking local session: $e');
      }

      // Session is no longer valid, clear local state
      await MpcSessionStore.clearSession();
    }

    // Also check backend for sessions we might not know about locally
    try {
      final result = await MpcApi.listPendingSessions();
      if (result.isSuccess && result.data != null && result.data!.isNotEmpty) {
        // Find the most recent sign session
        for (final raw in result.data!) {
          final session = PendingSession.fromJson(
            Map<String, dynamic>.from(raw as Map),
          );
          if (session.isResumableSign && !session.isExpired) {
            _currentPending = session;
            _pendingController.add(session);
            return session;
          }
        }
      }
    } catch (e) {
      print('[PendingSignService] Error checking backend sessions: $e');
    }

    _currentPending = null;
    _pendingController.add(null);
    return null;
  }

  /// Dismiss the pending session (user chose not to resume).
  /// Aborts the session on the backend.
  Future<void> dismiss() async {
    if (_currentPending != null) {
      await MpcApi.abortSession(_currentPending!.sessionId);
      await MpcSessionStore.clearSession();
      _currentPending = null;
      _pendingController.add(null);
    }
  }

  /// Clear local pending state without aborting the backend session.
  Future<void> clear() async {
    _currentPending = null;
    _pendingController.add(null);
    await MpcSessionStore.clearSession();
  }

  void dispose() {
    _pendingController.close();
  }
}
