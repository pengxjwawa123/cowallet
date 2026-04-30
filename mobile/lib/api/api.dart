/// Cowallet API 统一导出文件
///
/// 后端API架构概览：
/// - 认证系统：/api/v1/auth/* (设备注册/登录，无需密码)
/// - 价格系统：/api/v1/price/* (代币价格、历史K线)
/// - 交易系统：/api/v1/tx/* (提交交易、历史记录、模拟)
/// - AI助手：/api/v1/ai/* (聊天、意图分类)
/// - MPC计算：/api/v1/mpc/* (多方计算会话、消息传递)
/// - 策略引擎：/api/v1/policy/* (交易策略CRUD、评估)
///
/// 重要提示：
/// - 钱包余额、私钥/助记词管理应该在本地完成，不要上传到服务器
/// - 交易签名请在本地使用web3dart等库完成，只将签名后的rawTx提交给后端
///
/// 使用方式：
/// import 'package:cowallet/api/api.dart';
///
/// // 认证API
/// await AuthApi.register(deviceId: "xxx");
/// await AuthApi.login(deviceId: "xxx");
///
/// // 价格API
/// await PriceApi.getPrices(["ETH", "BTC"]);
///
/// // 交易API
/// await TxApi.submit(rawTx: "0x...");
/// await TxApi.getHistory();
///
/// // AI API
/// await AiApi.chat(message: "查看我的余额");
///
/// // 策略API
/// await PolicyApi.getPolicies();
///
/// // MPC API
/// await MpcApi.createSession(sessionType: "keygen", parties: [1, 2]);

export 'auth_api.dart';
export 'price_api.dart';
export 'tx_api.dart';
export 'ai_api.dart';
export 'policy_api.dart';
export 'mpc_api.dart';
export 'common_api.dart';
export 'wallet_api.dart';

