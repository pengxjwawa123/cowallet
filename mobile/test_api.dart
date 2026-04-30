// API测试文件，直接运行可以测试后端连通性
import 'lib/api/common_api.dart';
import 'lib/config/api_config.dart';

void main() async {
  print("🚀 开始测试API连通性...");
  print("📍 API地址: ${ApiConfig.baseUrl}");

  bool isOk = await CommonApi.healthCheck();

  if (isOk) {
    print("✅ API连接正常！");
    print("🎉 后端服务运行正常，可以开始开发啦！");
  } else {
    print("❌ API连接失败！");
    print("⚠️  请检查：");
    print("1. 后端服务是否正常运行");
    print("2. API地址是否正确: ${ApiConfig.baseUrl}");
    print("3. 网络是否可以访问服务器");
    print("4. 服务器防火墙是否开放3000端口");
  }
}
