import '../network/dio_client.dart';
import '../network/result.dart';

/// Swap/DEX API — calls backend 0x aggregator endpoints.
///
/// GET  /swap/quote  — Get swap price quote
/// POST /swap/build  — Build swap transaction calldata
class SwapApi {
  /// Get a swap quote (price + estimated output).
  ///
  /// [chainId] Target chain (e.g. 8453 for Base)
  /// [sellToken] Token symbol or contract address to sell
  /// [buyToken] Token symbol or contract address to buy
  /// [sellAmount] Human-readable sell amount (e.g. "0.5")
  static Future<Result<Map<String, dynamic>>> getQuote({
    required int chainId,
    required String sellToken,
    required String buyToken,
    required String sellAmount,
  }) async {
    return await DioClient.get(
      "/swap/quote",
      queryParameters: {
        "chain_id": chainId,
        "sell_token": sellToken,
        "buy_token": buyToken,
        "sell_amount": sellAmount,
      },
    );
  }

  /// Build a swap transaction (returns calldata ready to sign).
  ///
  /// [chainId] Target chain
  /// [sellToken] Token to sell
  /// [buyToken] Token to buy
  /// [sellAmount] Human-readable sell amount
  /// [slippage] Slippage tolerance in percent (default 0.5)
  /// [takerAddress] The wallet address executing the swap
  ///
  /// Returns: { to, data, value, gas_estimate, sell_token, buy_token,
  ///            sell_amount, buy_amount, price, allowance_target, chain_id }
  static Future<Result<Map<String, dynamic>>> buildSwapTx({
    required int chainId,
    required String sellToken,
    required String buyToken,
    required String sellAmount,
    required String takerAddress,
    double slippage = 0.5,
  }) async {
    return await DioClient.post(
      "/swap/build",
      data: {
        "chain_id": chainId,
        "sell_token": sellToken,
        "buy_token": buyToken,
        "sell_amount": sellAmount,
        "taker_address": takerAddress,
        "slippage": slippage,
      },
    );
  }
}
