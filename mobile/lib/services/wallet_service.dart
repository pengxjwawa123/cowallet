abstract class WalletService {
  Future<String> getAddress();
  Future<bool> hasWallet();
  Future<void> deleteWallet();

  /// MPC distributed signature: returns 65 bytes (r[32] || s[32] || v[1])
  Future<List<int>> sign(List<int> msgHash);
}
