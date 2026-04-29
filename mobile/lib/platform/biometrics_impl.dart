import 'package:flutter/services.dart';
import 'package:local_auth/local_auth.dart';
import 'biometrics.dart';

class LocalAuthBiometricService implements BiometricService {
  final _auth = LocalAuthentication();

  @override
  Future<bool> isAvailable() async {
    try {
      final canCheck = await _auth.canCheckBiometrics;
      final isSupported = await _auth.isDeviceSupported();
      return canCheck || isSupported;
    } on PlatformException {
      return false;
    }
  }

  @override
  Future<bool> authenticate({required String reason}) async {
    try {
      return await _auth.authenticate(
        localizedReason: reason,
        options: const AuthenticationOptions(
          biometricOnly: false,
        ),
      );
    } on PlatformException {
      return false;
    }
  }
}
