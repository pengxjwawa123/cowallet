class ApiConfig {
  // 后端API地址
  static const String baseUrl = "http://43.163.101.37:3000";

  // API统一前缀
  static const String apiPrefix = "/api/v1";

  // 完整API基础地址
  static const String apiBaseUrl = "$baseUrl$apiPrefix";

  // 连接超时时间（秒）
  static const int connectTimeout = 15;

  // 响应超时时间（秒）
  static const int receiveTimeout = 15;

  // 健康检查接口
  static const String healthCheck = "$baseUrl/health";
}
