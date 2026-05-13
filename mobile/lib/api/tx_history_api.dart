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
    required int chainId,
  }) async {
    return await DioClient.get<Map<String, dynamic>>(
      '/tx/tx-history',
      queryParameters: {
        'address': address,
        'chain_id': chainId,
      },
    );
  }

  /// Get transaction history across all chains for an address from the backend
  ///
  /// Returns a map with:
  /// - chains: List<Map> with { chain_id, chain_name, transactions: [...] }
  /// - total: int (total across all chains)
  static Future<Result<Map<String, dynamic>>> getAllHistory({
    required String address,
    int? limit,
  }) async {
    final params = <String, dynamic>{'address': address};
    if (limit != null) {
      params['limit'] = limit;
    }
    return await DioClient.get<Map<String, dynamic>>(
      '/tx/all-history',
      queryParameters: params,
    );
  }
}
