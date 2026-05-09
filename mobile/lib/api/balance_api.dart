import '../network/dio_client.dart';
import '../network/result.dart';

class BalanceApi {
  /// Get token balances for an address from the backend (via Covalent)
  ///
  /// Returns a map with:
  /// - address: String
  /// - chain_id: int
  /// - tokens: List<Map> with { symbol, balance, usd, native }
  /// - total_usd: String
  static Future<Result<Map<String, dynamic>>> getBalance({
    required String address,
    int chainId = 84532,
  }) async {
    return await DioClient.get<Map<String, dynamic>>(
      '/balance',
      queryParameters: {
        'address': address,
        'chain_id': chainId,
      },
    );
  }
}
