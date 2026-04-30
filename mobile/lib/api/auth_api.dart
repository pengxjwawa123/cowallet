import '../network/dio_client.dart';
import '../network/result.dart';
import '../utils/secure_storage.dart';

/// 认证API - 匹配后端实际接口
class AuthApi {
  /// 注册新用户
  /// [deviceId] 设备唯一标识
  /// [email] 可选邮箱
  /// 返回 token 和 user_id
  static Future<Result<Map<String, dynamic>>> register({
    required String deviceId,
    String? email,
  }) async {
    Result<Map<String, dynamic>> result = await DioClient.post(
      "/auth/register",
      data: {
        "device_id": deviceId,
        if (email != null) "email": email,
      },
    );

    // 注册成功自动存储token
    if (result.isSuccess) {
      String? token = result.data?["token"];
      String? userId = result.data?["user_id"];
      
      print("📝 AuthApi.register response: token=${token?.substring(0, 30)}..., userId=$userId");
      
      if (token != null) {
        await SecureStorage.saveToken(token);
        print("✅ Token saved to SecureStorage");
      } else {
        print("❌ Token is null in response");
      }
      
      if (userId != null) {
        await SecureStorage.saveUserId(userId);
        print("✅ UserId saved to SecureStorage");
      }
    } else {
      print("❌ Registration failed: ${result.errorMessage}");
    }
    return result;
  }

  /// 使用设备ID登录
  /// [deviceId] 设备唯一标识
  /// 返回 token 和 user_id
  static Future<Result<Map<String, dynamic>>> login({
    required String deviceId,
  }) async {
    Result<Map<String, dynamic>> result = await DioClient.post(
      "/auth/login",
      data: {"device_id": deviceId},
    );

    // 登录成功自动存储token
    if (result.isSuccess) {
      String? token = result.data?["token"];
      String? userId = result.data?["user_id"];
      if (token != null) {
        await SecureStorage.saveToken(token);
      }
      if (userId != null) {
        await SecureStorage.saveUserId(userId);
      }
    }
    return result;
  }

  /// 获取当前会话信息
  static Future<Result<Map<String, dynamic>>> getSessionInfo() async {
    return await DioClient.get("/auth/session");
  }

  /// 退出登录 - 清除本地所有数据
  static Future<void> logout() async {
    await SecureStorage.clearAll();
  }

  /// 检查是否已登录（本地有token）
  static Future<bool> isLoggedIn() async {
    String? token = await SecureStorage.getToken();
    return token != null && token.isNotEmpty;
  }
}
