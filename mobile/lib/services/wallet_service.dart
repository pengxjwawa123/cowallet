class SignResult {
  final List<int> signature;
  final String? sessionId;

  SignResult({required this.signature, this.sessionId});
}

abstract class WalletService {
  Future<String> getAddress();
  Future<bool> hasWallet();
  Future<void> deleteWallet();

  /// MPC distributed signature: returns 65 bytes (r[32] || s[32] || v[1])
  Future<List<int>> sign(List<int> msgHash);

  /// MPC distributed signature with session tracking
  Future<SignResult> signWithSession(List<int> msgHash);
}
