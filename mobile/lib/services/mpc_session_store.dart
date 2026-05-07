import 'dart:convert';
import '../utils/secure_storage.dart';

/// MPC session state for recovery after interruption.
/// Stores metadata only - crypto secrets remain in Rust memory.
class MpcSessionState {
  final String sessionId;
  final String remoteSessionId;
  final String sessionType; // 'dkg', 'sign', 'reshare', 'presign'
  final int currentRound;
  final DateTime createdAt;
  final int lastMessageId;
  final Map<String, dynamic>? metadata; // Additional session-specific data

  MpcSessionState({
    required this.sessionId,
    required this.remoteSessionId,
    required this.sessionType,
    required this.currentRound,
    required this.createdAt,
    this.lastMessageId = 0,
    this.metadata,
  });

  Map<String, dynamic> toJson() => {
        'session_id': sessionId,
        'remote_session_id': remoteSessionId,
        'session_type': sessionType,
        'current_round': currentRound,
        'created_at': createdAt.toIso8601String(),
        'last_message_id': lastMessageId,
        'metadata': metadata,
      };

  factory MpcSessionState.fromJson(Map<String, dynamic> json) {
    return MpcSessionState(
      sessionId: json['session_id'] as String,
      remoteSessionId: json['remote_session_id'] as String,
      sessionType: json['session_type'] as String,
      currentRound: json['current_round'] as int,
      createdAt: DateTime.parse(json['created_at'] as String),
      lastMessageId: json['last_message_id'] as int? ?? 0,
      metadata: json['metadata'] as Map<String, dynamic>?,
    );
  }

  MpcSessionState copyWith({
    String? sessionId,
    String? remoteSessionId,
    String? sessionType,
    int? currentRound,
    DateTime? createdAt,
    int? lastMessageId,
    Map<String, dynamic>? metadata,
  }) {
    return MpcSessionState(
      sessionId: sessionId ?? this.sessionId,
      remoteSessionId: remoteSessionId ?? this.remoteSessionId,
      sessionType: sessionType ?? this.sessionType,
      currentRound: currentRound ?? this.currentRound,
      createdAt: createdAt ?? this.createdAt,
      lastMessageId: lastMessageId ?? this.lastMessageId,
      metadata: metadata ?? this.metadata,
    );
  }
}

/// Persistent storage for MPC session state to enable recovery.
class MpcSessionStore {
  static const String _keyActiveSession = 'mpc_active_session';

  /// Save current session state for recovery.
  static Future<void> saveSession(MpcSessionState state) async {
    final json = jsonEncode(state.toJson());
    await SecureStorage.save(_keyActiveSession, json);
  }

  /// Load active session state if exists.
  static Future<MpcSessionState?> loadSession() async {
    final json = await SecureStorage.get(_keyActiveSession);
    if (json == null || json.isEmpty) return null;

    try {
      final map = jsonDecode(json) as Map<String, dynamic>;
      return MpcSessionState.fromJson(map);
    } catch (e) {
      // Corrupted state, clear it
      await clearSession();
      return null;
    }
  }

  /// Clear active session state (called after successful completion or fatal error).
  static Future<void> clearSession() async {
    await SecureStorage.delete(_keyActiveSession);
  }

  /// Check if there's an active session that might be resumable.
  static Future<bool> hasActiveSession() async {
    final session = await loadSession();
    if (session == null) return false;

    // Check if session is too old (>5 minutes = expired)
    final age = DateTime.now().difference(session.createdAt);
    if (age.inMinutes > 5) {
      await clearSession();
      return false;
    }

    return true;
  }

  /// Update the last message ID for incremental polling.
  static Future<void> updateLastMessageId(int messageId) async {
    final session = await loadSession();
    if (session == null) return;

    final updated = session.copyWith(lastMessageId: messageId);
    await saveSession(updated);
  }

  /// Update the current round.
  static Future<void> updateCurrentRound(int round) async {
    final session = await loadSession();
    if (session == null) return;

    final updated = session.copyWith(currentRound: round);
    await saveSession(updated);
  }
}

/// Exception thrown when an MPC session is interrupted and needs recovery.
class MpcSessionInterruptedException implements Exception {
  final String message;
  final MpcSessionState? sessionState;

  MpcSessionInterruptedException(this.message, {this.sessionState});

  @override
  String toString() => 'MpcSessionInterruptedException: $message';
}
