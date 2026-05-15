import 'dart:async';

import '../api/mpc_api.dart';
import 'locator.dart';

/// Automatic presignature pool refill service.
///
/// Monitors available presignature count and triggers background generation
/// when the pool drops below the configured threshold. Debounces to prevent
/// concurrent refill attempts.
class PresignPoolService {
  /// Minimum presignatures before auto-refill triggers.
  final int threshold;

  /// Target number of presignatures to maintain (generates up to this count).
  final int refillTarget;

  /// How often to poll presign status (periodic check).
  final Duration checkInterval;

  Timer? _periodicTimer;
  bool _isRefilling = false;
  bool _disposed = false;

  PresignPoolService({
    this.threshold = 3,
    this.refillTarget = 5,
    this.checkInterval = const Duration(minutes: 5),
  });

  /// Start periodic background monitoring.
  /// Should be called after the wallet is confirmed to exist.
  void start() {
    if (_disposed) return;
    _periodicTimer?.cancel();
    _periodicTimer = Timer.periodic(checkInterval, (_) => checkAndRefill());
    // Run an initial check immediately.
    checkAndRefill();
  }

  /// Stop periodic monitoring and cancel any pending timers.
  void dispose() {
    _disposed = true;
    _periodicTimer?.cancel();
    _periodicTimer = null;
  }

  /// Check presign pool and refill if below threshold.
  /// Safe to call from anywhere (debounced internally).
  Future<void> checkAndRefill() async {
    if (_isRefilling || _disposed) return;

    try {
      final hasWallet = await Services.mpcWallet.hasWallet();
      if (!hasWallet) return;

      final address = await Services.mpcWallet.getAddress();
      final result = await MpcApi.getPresignStatus(address);

      if (!result.isSuccess || result.data == null) return;

      final availableCount = result.data!['available_count'] as int? ?? 0;

      if (availableCount < threshold) {
        final needed = refillTarget - availableCount;
        if (needed > 0) {
          await _refill(address, needed);
        }
      }
    } catch (e) {
      print('[PresignPoolService] Check failed: $e');
    }
  }

  /// Internal refill - generates presignatures in background.
  Future<void> _refill(String walletAddress, int count) async {
    if (_isRefilling || _disposed) return;
    _isRefilling = true;

    try {
      print('[PresignPoolService] Refilling: generating $count presignatures');
      final generated = await Services.mpcWallet.runPresign(
        walletId: walletAddress,
        count: count,
      );
      print('[PresignPoolService] Refill complete: generated $generated/$count');
    } catch (e) {
      print('[PresignPoolService] Refill error: $e');
    } finally {
      _isRefilling = false;
    }
  }

  /// Whether a refill is currently in progress.
  bool get isRefilling => _isRefilling;
}
