abstract class BiometricService {
  Future<bool> isAvailable();
  Future<bool> authenticate({required String reason});
}

class BiometricServiceStub implements BiometricService {
  @override
  Future<bool> isAvailable() async => true;

  @override
  Future<bool> authenticate({required String reason}) async => true;
}
