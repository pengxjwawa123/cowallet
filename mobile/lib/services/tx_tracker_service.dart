import 'dart:async';

import '../api/tx_api.dart';

/// Transaction confirmation status.
enum TxStatus {
  pending,
  broadcast,
  confirmed,
  failed,
}

/// Parsed transaction status response from the backend.
class TxStatusInfo {
  final String txHash;
  final TxStatus status;
  final int? blockNumber;
  final int? gasUsed;
  final int? confirmations;
  final String? confirmedAt;

  TxStatusInfo({
    required this.txHash,
    required this.status,
    this.blockNumber,
    this.gasUsed,
    this.confirmations,
    this.confirmedAt,
  });

  factory TxStatusInfo.fromJson(Map<String, dynamic> json) {
    return TxStatusInfo(
      txHash: json['tx_hash'] as String,
      status: _parseStatus(json['status'] as String),
      blockNumber: json['block_number'] as int?,
      gasUsed: json['gas_used'] as int?,
      confirmations: json['confirmations'] as int?,
      confirmedAt: json['confirmed_at'] as String?,
    );
  }

  static TxStatus _parseStatus(String status) {
    switch (status) {
      case 'confirmed':
        return TxStatus.confirmed;
      case 'failed':
        return TxStatus.failed;
      case 'broadcast':
        return TxStatus.broadcast;
      default:
        return TxStatus.pending;
    }
  }

  bool get isFinal => status == TxStatus.confirmed || status == TxStatus.failed;
}

/// Service that tracks transaction confirmations by polling the backend.
///
/// After a transaction is broadcast, call [track] with the tx hash.
/// The service polls every 5 seconds until the transaction reaches a
/// final state (confirmed or failed), then invokes the callback.
class TxTrackerService {
  final Map<String, Timer> _timers = {};
  final Map<String, StreamController<TxStatusInfo>> _controllers = {};

  /// Start tracking a transaction.
  ///
  /// Returns a stream that emits [TxStatusInfo] on each poll until the
  /// transaction reaches a final state. The stream automatically closes
  /// when tracking is complete.
  Stream<TxStatusInfo> track(String txHash) {
    // Don't create duplicate trackers
    if (_controllers.containsKey(txHash)) {
      return _controllers[txHash]!.stream;
    }

    final controller = StreamController<TxStatusInfo>.broadcast();
    _controllers[txHash] = controller;

    // Start polling every 5 seconds
    _timers[txHash] = Timer.periodic(
      const Duration(seconds: 5),
      (_) => _poll(txHash),
    );

    // Also poll immediately
    _poll(txHash);

    return controller.stream;
  }

  /// Stop tracking a transaction.
  void cancel(String txHash) {
    _timers[txHash]?.cancel();
    _timers.remove(txHash);
    _controllers[txHash]?.close();
    _controllers.remove(txHash);
  }

  /// Cancel all active trackers.
  void dispose() {
    for (final timer in _timers.values) {
      timer.cancel();
    }
    _timers.clear();
    for (final controller in _controllers.values) {
      controller.close();
    }
    _controllers.clear();
  }

  Future<void> _poll(String txHash) async {
    final result = await TxApi.getStatus(txHash);

    if (!result.isSuccess || result.data == null) {
      return; // Silently ignore poll failures, will retry next interval
    }

    final info = TxStatusInfo.fromJson(result.data!);
    final controller = _controllers[txHash];
    if (controller == null || controller.isClosed) return;

    controller.add(info);

    // Stop polling once we reach a final state
    if (info.isFinal) {
      cancel(txHash);
    }
  }
}
