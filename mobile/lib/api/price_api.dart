import '../network/dio_client.dart';
import '../network/result.dart';

/// 价格行情API - 匹配后端实际接口
class PriceApi {
  /// 获取多个代币的当前价格
  /// [symbols] 代币符号列表，如 ["ETH", "BTC", "USDC"]
  /// 返回价格列表，包含USD价格和24h涨跌幅
  static Future<Result<List<dynamic>>> getPrices(List<String> symbols) async {
    Result<Map<String, dynamic>> result = await DioClient.get(
      "/price",
      params: {"tokens": symbols.join(",")},
    );

    if (result.isSuccess) {
      List<dynamic> prices = result.data?["prices"] ?? [];
      return Result.success(prices);
    }
    return Result.error(result.errorMessage ?? "获取价格失败", -1);
  }

  /// 获取单个代币的历史价格数据
  /// [symbol] 代币符号，如 "ETH"
  /// [days] 天数，默认7天，最大90天
  /// 返回价格历史K线数据
  static Future<Result<Map<String, dynamic>>> getPriceHistory({
    required String symbol,
    int days = 7,
  }) async {
    return await DioClient.get(
      "/price/history",
      params: {
        "token": symbol,
        "days": days,
      },
    );
  }

  /// 快捷获取ETH价格
  static Future<double?> getEthPrice() async {
    Result<List<dynamic>> result = await getPrices(["ETH"]);
    if (result.isSuccess && result.data != null && result.data!.isNotEmpty) {
      return result.data!.first["usd"] as double;
    }
    return null;
  }

  /// 获取常用代币的价格（ETH, USDC, BTC, USDT, DAI）
  static Future<Result<List<dynamic>>> getPopularPrices() async {
    return await getPrices(["ETH", "USDC", "BTC", "USDT", "DAI"]);
  }
}
