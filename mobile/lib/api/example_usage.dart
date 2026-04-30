/// API 接口使用示例
/// 本文件展示了所有API的调用方式和使用场景

import 'api.dart';
import '../utils/secure_storage.dart';

/// ==================== 认证相关示例 ====================
class AuthExample {
  /// 首次启动：注册设备
  static Future<void> registerDevice() async {
    // 1. 生成/获取设备唯一标识（实际项目中用 device_info_plus 包）
    String deviceId = "device_unique_id_here"; // 替换为实际设备ID

    // 2. 调用注册接口
    var result = await AuthApi.register(deviceId: deviceId);

    if (result.isSuccess) {
      print("注册成功！用户ID: ${result.data?["user_id"]}");
      print("Token: ${result.data?["token"]}");
      // token已自动保存到SecureStorage
    } else {
      print("注册失败: ${result.errorMessage}");
    }
  }

  /// 已有设备：登录
  static Future<void> loginDevice() async {
    String? deviceId = await SecureStorage.getDeviceId();
    if (deviceId == null) {
      print("设备ID不存在，请先注册");
      return;
    }

    var result = await AuthApi.login(deviceId: deviceId);

    if (result.isSuccess) {
      print("登录成功！");
    } else {
      print("登录失败: ${result.errorMessage}");
    }
  }

  /// 检查登录状态
  static Future<void> checkLoginStatus() async {
    bool isLoggedIn = await AuthApi.isLoggedIn();
    print("是否已登录: $isLoggedIn");

    if (isLoggedIn) {
      // 验证会话状态
      var result = await AuthApi.getSessionInfo();
      if (result.isSuccess) {
        print("会话有效: ${result.data}");
      } else {
        print("会话已过期，请重新登录");
        await SecureStorage.clearAll();
      }
    }
  }

  /// 退出登录
  static Future<void> logout() async {
    await AuthApi.logout();
    print("已退出登录");
  }
}

/// ==================== 价格相关示例 ====================
class PriceExample {
  /// 获取常用代币价格
  static Future<void> getPrices() async {
    var result = await PriceApi.getPopularPrices();

    if (result.isSuccess) {
      for (var price in result.data!) {
        print("${price["symbol"]}: \$${price["usd"]} (${price["change_24h"]}%)");
      }
    }
  }

  /// 获取ETH价格历史K线
  static Future<void> getEthPriceHistory() async {
    var result = await PriceApi.getPriceHistory(symbol: "ETH", days: 7);

    if (result.isSuccess) {
      print("${result.data?["symbol"]} 价格历史:");
      var prices = result.data?["prices"] as List;
      print("共 ${prices.length} 个数据点");
    }
  }
}

/// ==================== 交易相关示例 ====================
class TxExample {
  /// 提交转账交易
  static Future<void> submitTransaction() async {
    // 注意：实际项目中需要在本地签名，私钥不上传服务器
    String signedTx = "0x签名后的交易数据";

    var result = await TxApi.submit(
      rawTx: signedTx,
      chainId: 84532, // Base Sepolia
      toAddr: "0x接收地址",
      value: "100000000000000000", // 0.1 ETH
      token: "ETH",
    );

    if (result.isSuccess) {
      print("交易已提交！Hash: ${result.data?["tx_hash"]}");
    } else {
      print("交易失败: ${result.errorMessage}");
    }
  }

  /// 获取交易历史
  static Future<void> getTransactionHistory() async {
    var result = await TxApi.getHistory(limit: 20, offset: 0);

    if (result.isSuccess) {
      print("共 ${result.data!.length} 笔交易:");
      for (var tx in result.data!) {
        print("${tx["tx_hash"]} - ${tx["status"]} - ${tx["created_at"]}");
      }
    }
  }

  /// 模拟合约调用
  static Future<void> simulateCall() async {
    var result = await TxApi.simulate(
      to: "0x合约地址",
      data: "0x合约调用数据",
    );

    if (result.isSuccess) {
      if (result.data?["success"] == true) {
        print("模拟成功，返回数据: ${result.data?["return_data"]}");
      } else {
        print("模拟失败: ${result.data?["return_data"]}");
      }
    }
  }
}

/// ==================== AI 助手示例 ====================
class AiExample {
  /// 与AI聊天
  static Future<void> chatWithAi() async {
    List<Map<String, dynamic>> history = [
      {"role": "user", "content": "我的钱包里有多少钱？"},
      {"role": "assistant", "content": "您当前有 1.5 ETH，价值约 4500 美元"},
    ];

    var result = await AiApi.chat(
      message: "最近价格走势怎么样？",
      history: history,
    );

    if (result.isSuccess) {
      print("AI回复: ${result.data?["message"]}");
    }
  }

  /// 识别用户意图
  static Future<void> detectIntent() async {
    var result = await AiApi.classifyIntent("给0x123转0.1个ETH");

    if (result.isSuccess) {
      print("识别到的意图: ${result.data}");
    }
  }
}

/// ==================== 策略相关示例 ====================
class PolicyExample {
  /// 获取所有策略
  static Future<void> listPolicies() async {
    var result = await PolicyApi.getPolicies();

    if (result.isSuccess) {
      print("共有 ${result.data!.length} 个策略:");
      for (var policy in result.data!) {
        print("- ${policy["name"]}: ${policy["enabled"] ? "已启用" : "已禁用"}");
      }
    }
  }

  /// 创建大额转账确认策略
  static Future<void> createLargeAmountPolicy() async {
    var policy = PolicyApi.templateLargeAmountConfirm(
      name: "大额转账提醒",
      thresholdUsd: 500, // 超过500美元需要确认
    );

    var result = await PolicyApi.createPolicy(
      name: policy["name"],
      description: policy["description"],
      rules: policy["rules"],
      action: policy["action"],
      enabled: policy["enabled"],
      priority: policy["priority"],
    );

    if (result.isSuccess) {
      print("策略创建成功: ${result.data?["id"]}");
    }
  }

  /// 评估交易是否符合策略
  static Future<void> evaluateTx() async {
    var result = await PolicyApi.evaluateTransaction(
      txData: {
        "from": "0x发送地址",
        "to": "0x接收地址",
        "value": "1000000000000000000",
        "chain_id": 84532,
      },
    );

    if (result.isSuccess) {
      print("评估结果: ${result.data}");
    }
  }
}

/// ==================== MPC 示例 ====================
class MpcExample {
  /// 创建密钥生成会话
  static Future<void> createKeygenSession() async {
    var result = await MpcApi.createSession(
      sessionType: "keygen",
      parties: [1, 2, 3], // 3方参与
      threshold: 2, // 2/3 门限
    );

    if (result.isSuccess) {
      String sessionId = result.data?["session_id"];
      print("MPC会话创建成功: $sessionId");
      // 接下来可以发送/接收消息进行协议交互
    }
  }

  /// 发送MPC消息
  static Future<void> sendMpcMessage(String sessionId) async {
    await MpcApi.sendMessage(
      sessionId: sessionId,
      fromParty: 1,
      toParty: 2,
      round: 1,
      payload: [0x01, 0x02, 0x03], // 加密的协议消息
    );
  }
}

/// 完整启动流程示例
class AppStartupFlow {
  static Future<void> startup() async {
    print("=== 应用启动流程 ===");

    // 1. 检查后端健康状态
    bool isBackendOk = await CommonApi.healthCheck();
    if (!isBackendOk) {
      print("后端服务不可用，请检查网络连接");
      return;
    }
    print("✓ 后端服务正常");

    // 2. 检查登录状态
    bool isLoggedIn = await AuthApi.isLoggedIn();
    if (!isLoggedIn) {
      print("未登录，开始注册设备...");
      await AuthExample.registerDevice();
    } else {
      print("✓ 已登录，验证会话...");
      await AuthExample.checkLoginStatus();
    }

    // 3. 加载价格数据
    print("加载市场价格...");
    await PriceExample.getPrices();

    // 4. 加载交易历史
    print("加载交易历史...");
    await TxExample.getTransactionHistory();

    print("=== 启动流程完成 ===");
  }
}
