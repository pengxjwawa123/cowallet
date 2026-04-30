import '../network/dio_client.dart';
import '../network/result.dart';

/// 交易策略API - 匹配后端实际接口
class PolicyApi {
  /// 获取所有策略列表
  static Future<Result<List<dynamic>>> getPolicies() async {
    Result<Map<String, dynamic>> result = await DioClient.get("/policy");

    if (result.isSuccess) {
      List<dynamic> policies = result.data?["policies"] ?? [];
      return Result.success(policies);
    }
    return Result.error(result.errorMessage ?? "获取策略列表失败", -1);
  }

  /// 获取单个策略详情
  /// [policyId] 策略ID
  static Future<Result<Map<String, dynamic>>> getPolicy(String policyId) async {
    return await DioClient.get("/policy/$policyId");
  }

  /// 创建新策略
  /// [name] 策略名称
  /// [description] 策略描述
  /// [rules] 规则定义（JSON格式）
  /// [action] 触发动作（allow, deny, confirm等）
  /// [enabled] 是否启用
  /// [priority] 优先级，数字越大优先级越高
  static Future<Result<Map<String, dynamic>>> createPolicy({
    required String name,
    String? description,
    required Map<String, dynamic> rules,
    required Map<String, dynamic> action,
    bool enabled = true,
    int priority = 0,
  }) async {
    return await DioClient.post(
      "/policy",
      data: {
        "name": name,
        if (description != null) "description": description,
        "rules": rules,
        "action": action,
        "enabled": enabled,
        "priority": priority,
      },
    );
  }

  /// 更新策略
  static Future<Result<Map<String, dynamic>>> updatePolicy({
    required String policyId,
    String? name,
    String? description,
    Map<String, dynamic>? rules,
    Map<String, dynamic>? action,
    bool? enabled,
    int? priority,
  }) async {
    Map<String, dynamic> data = {};
    if (name != null) data["name"] = name;
    if (description != null) data["description"] = description;
    if (rules != null) data["rules"] = rules;
    if (action != null) data["action"] = action;
    if (enabled != null) data["enabled"] = enabled;
    if (priority != null) data["priority"] = priority;

    return await DioClient.put("/policy/$policyId", data: data);
  }

  /// 删除策略
  static Future<Result<bool>> deletePolicy(String policyId) async {
    Result result = await DioClient.delete("/policy/$policyId");
    return Result.success(result.isSuccess);
  }

  /// 评估交易是否符合策略
  /// [txData] 交易数据，包含from, to, value, chainId等信息
  /// 返回评估结果
  static Future<Result<Map<String, dynamic>>> evaluateTransaction({
    required Map<String, dynamic> txData,
  }) async {
    return await DioClient.post(
      "/policy/evaluate",
      data: txData,
    );
  }

  // ==================== 常用策略模板 ====================

  /// 创建大额转账确认策略
  /// [thresholdUsd] 阈值金额（美元），超过需要确认
  static Map<String, dynamic> templateLargeAmountConfirm({
    String name = "大额转账确认",
    String description = "大额转账需要二次确认",
    double thresholdUsd = 1000,
    int priority = 100,
  }) {
    return {
      "name": name,
      "description": description,
      "rules": {
        "type": "amount_threshold",
        "threshold_usd": thresholdUsd,
      },
      "action": {
        "type": "confirm",
        "message": "转账金额较大，请确认",
      },
      "enabled": true,
      "priority": priority,
    };
  }

  /// 创建白名单地址策略
  /// [allowedAddresses] 允许的地址列表
  static Map<String, dynamic> templateWhitelist({
    String name = "地址白名单",
    String description = "只允许转账到白名单地址",
    required List<String> allowedAddresses,
    int priority = 200,
  }) {
    return {
      "name": name,
      "description": description,
      "rules": {
        "type": "whitelist",
        "allowed_addresses": allowedAddresses,
      },
      "action": {
        "type": "deny",
        "message": "该地址不在白名单中",
      },
      "enabled": true,
      "priority": priority,
    };
  }

  /// 创建每日限额策略
  /// [maxDailyUsd] 每日最大限额（美元）
  static Map<String, dynamic> templateDailyLimit({
    String name = "每日限额",
    String description = "每日累计转账限额",
    double maxDailyUsd = 10000,
    int priority = 150,
  }) {
    return {
      "name": name,
      "description": description,
      "rules": {
        "type": "daily_limit",
        "max_daily_usd": maxDailyUsd,
      },
      "action": {
        "type": "deny",
        "message": "今日转账已超过限额",
      },
      "enabled": true,
      "priority": priority,
    };
  }
}
