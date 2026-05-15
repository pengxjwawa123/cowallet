import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:local_auth/local_auth.dart';
import 'biometrics.dart';
import 'secure_storage_impl.dart';

class LocalAuthBiometricService implements BiometricService {
  final _auth = LocalAuthentication();
  final _storage = FlutterSecureStorageService();
  static const _enabledKey = 'biometric_enabled';

  void _debug(String message) {
    if (kDebugMode) {
      print('[BiometricDebug] $message');
    }
  }

  @override
  Future<bool> isAvailable() async {
    try {
      final canCheck = await _auth.canCheckBiometrics;
      final isSupported = await _auth.isDeviceSupported();
      _debug('isAvailable: canCheckBiometrics=$canCheck, isDeviceSupported=$isSupported');
      return canCheck || isSupported;
    } on PlatformException catch (e) {
      _debug('isAvailable error: ${e.code}, ${e.message}');
      return false;
    }
  }

  @override
  Future<List<String>> getAvailableBiometrics() async {
    try {
      final types = await _auth.getAvailableBiometrics();
      _debug('getAvailableBiometrics: ${types.map((t) => t.name).toList()}');
      return types.map((t) => t.name).toList();
    } on PlatformException catch (e) {
      _debug('getAvailableBiometrics error: ${e.code}, ${e.message}');
      return [];
    }
  }

  /// Check if user has any biometric enrolled on the device
  @override
  Future<bool> hasEnrolledBiometrics() async {
    try {
      // On some devices getAvailableBiometrics returns empty even when enrolled
      // So we also check isDeviceSupported as a fallback
      final types = await _auth.getAvailableBiometrics();
      final isSupported = await _auth.isDeviceSupported();
      _debug('hasEnrolledBiometrics: types.length=${types.length}, types=$types, isDeviceSupported=$isSupported');

      // More lenient check: if device supports biometrics, assume user may have enrolled
      // The actual authentication will fail if not enrolled anyway
      return types.isNotEmpty || isSupported;
    } on PlatformException catch (e) {
      _debug('hasEnrolledBiometrics error: ${e.code}, ${e.message}');
      // Fallback: if we got here and device supports auth, return true
      return await _auth.isDeviceSupported();
    }
  }

  /// Get the primary biometric type name for display (e.g., Face ID, Fingerprint)
  @override
  Future<String> getPrimaryBiometricType() async {
    try {
      final types = await _auth.getAvailableBiometrics();
      _debug('getPrimaryBiometricType: $types');
      if (types.contains(BiometricType.face)) return 'Face ID';
      if (types.contains(BiometricType.fingerprint)) return 'Fingerprint';
      if (types.contains(BiometricType.iris)) return 'Iris';
      if (types.contains(BiometricType.strong)) return 'Biometric';
      if (types.contains(BiometricType.weak)) return 'Biometric';
      return 'Biometric';
    } on PlatformException catch (e) {
      _debug('getPrimaryBiometricType error: ${e.code}, ${e.message}');
      return 'Biometric';
    }
  }

  @override
  Future<bool> isEnabled() async {
    final stored = await _storage.read(_enabledKey);
    _debug('isEnabled: stored=$stored, result=${stored == 'true'}');
    return stored == 'true';
  }

  @override
  Future<void> setEnabled(bool enabled) async {
    _debug('setEnabled: $enabled');
    await _storage.write(_enabledKey, enabled ? 'true' : 'false');
  }

  @override
  Future<bool> authenticate({required String reason}) async {
    _debug('authenticate called with reason: $reason');
    try {
      final available = await isAvailable();
      _debug('authenticate: available=$available');
      if (!available) {
        _debug('authenticate: biometric not available');
        return false;
      }

      final hasEnrolled = await hasEnrolledBiometrics();
      _debug('authenticate: hasEnrolled=$hasEnrolled');
      if (!hasEnrolled) {
        _debug('authenticate: no biometric enrolled');
        return false;
      }

      _debug('Calling _auth.authenticate...');
      final result = await _auth.authenticate(
        localizedReason: reason,
        options: const AuthenticationOptions(
          biometricOnly: true,
          useErrorDialogs: true,
          stickyAuth: true,
        ),
      );
      _debug('authenticate result: $result');
      return result;
    } on PlatformException catch (e) {
      _debug('authenticate PlatformException: code=${e.code}, message=${e.message}, details=${e.details}');
      return false;
    } catch (e) {
      _debug('authenticate unexpected error: $e');
      return false;
    }
  }
}
