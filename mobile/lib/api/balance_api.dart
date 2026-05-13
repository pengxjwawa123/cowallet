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
    required int chainId,
  }) async {
    return await DioClient.get<Map<String, dynamic>>(
      '/balance',
      queryParameters: {
        'address': address,
        'chain_id': chainId,
      },
    );
  }

  /// Get token balances across all chains for an address from the backend (via GoldRush)
  ///
  /// Returns a map with:
  /// - address: String
  /// - chains: List<Map> with { chain_id, chain_name, tokens: [...], total_usd }
  /// - total_usd: String (sum across all chains)
  static Future<Result<Map<String, dynamic>>> getAllBalances({
    required String address,
  }) async {
    return await DioClient.get<Map<String, dynamic>>(
      '/balance/all',
      queryParameters: {
        'address': address,
      },
    );
  }
}
