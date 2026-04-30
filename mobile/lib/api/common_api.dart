import '../config/api_config.dart';
import '../network/dio_client.dart';

class CommonApi {
  // 健康检查，测试API连通性
  static Future<bool> healthCheck() async {
    try {
      var response = await DioClient.instance.get(ApiConfig.healthCheck);
      return response.data == "ok";
    } catch (e) {
      return false;
    }
  }
}
