import '../network/dio_client.dart';
import '../network/result.dart';

class TxHistoryApi {
  /// Get on-chain transaction history from the backend (via Covalent)
  ///
  /// Returns a map with:
  /// - transactions: List<Map> with { tx_hash, from, to, value, timestamp, status, gas_used, token_symbol, value_quote }
  /// - total: int
  static Future<Result<Map<String, dynamic>>> getHistory({
    required String address,
    int chainId = 84532,
  }) async {
    return await DioClient.get<Map<String, dynamic>>(
      '/tx/tx-history',
      queryParameters: {
        'address': address,
        'chain_id': chainId,
      },
    );
  }
}
