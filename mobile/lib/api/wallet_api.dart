/// 钱包工具类
/// 注意：后端没有专门的/wallet接口
/// - 交易历史请使用 TxApi.getHistory()
/// - 余额查询请在链上本地查询或通过PriceApi获取代币价格
/// - 钱包创建/导入请在本地使用web3dart等库完成，私钥不要上传服务器

import '../network/dio_client.dart';
import '../network/result.dart';
import 'tx_api.dart';
import 'price_api.dart';

export 'tx_api.dart' show TxApi;
export 'price_api.dart' show PriceApi;

/// 钱包工具类 - 提供便捷的组合方法
class WalletApi {
  /// 获取钱包总价值（通过交易历史和价格计算）
  /// 这是一个示例方法，实际余额应该在链上查询
  static Future<double> getEstimatedValue(String address) async {
    // 1. 获取交易历史
    var txResult = await TxApi.getHistory();
    if (!txResult.isSuccess) return 0;

    // 2. 获取ETH当前价格
    double? ethPrice = await PriceApi.getEthPrice();

    // 3. 计算估算价值（简化示例）
    // 实际项目中应该用web3dart在本地查询链上余额
    return ethPrice ?? 0;
  }
}
