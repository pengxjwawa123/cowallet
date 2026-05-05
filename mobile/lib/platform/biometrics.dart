abstract class BiometricService {
  /// Check if biometric authentication is available on this device
  Future<bool> isAvailable();

  /// Perform biometric authentication
  Future<bool> authenticate({required String reason});

  /// Get available biometric types (e.g., fingerprint, face ID)
  Future<List<String>> getAvailableBiometrics();

  /// Check if biometric authentication is enabled by user
  Future<bool> isEnabled();

  /// Enable or disable biometric authentication
  Future<void> setEnabled(bool enabled);

  /// Check if user has any biometric enrolled on the device
  Future<bool> hasEnrolledBiometrics();

  /// Get the primary biometric type name for display (e.g., Face ID, Fingerprint)
  Future<String> getPrimaryBiometricType();
}

class BiometricServiceStub implements BiometricService {
  bool _enabled = true;

  @override
  Future<bool> isAvailable() async => true;

  @override
  Future<bool> authenticate({required String reason}) async => true;

  @override
  Future<List<String>> getAvailableBiometrics() async => ['fingerprint', 'face'];

  @override
  Future<bool> isEnabled() async => _enabled;

  @override
  Future<void> setEnabled(bool enabled) async => _enabled = enabled;

  @override
  Future<bool> hasEnrolledBiometrics() async => true;

  @override
  Future<String> getPrimaryBiometricType() async => 'Face ID';
}
