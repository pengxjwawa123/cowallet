# Flutter 客户端认证问题根本诊断

## 🔴 问题现象

从 Flutter 日志可以看到：

```
// ✅ 第一个请求成功
POST /api/v1/auth/register
→ Response: {"token":"eyJ0...", "user_id":"2698..."}

// ❌ 第二个请求失败 (401 Unauthorized)
POST /api/v1/mpc/session
→ 错误：401 Unauthorized
→ 原因：缺少 Authorization header
```

## 🔍 根本原因分析

### 问题 1: Token 没有被发送到后端
从您的 curl 请求可以看出：

```bash
# ❌ 错误的请求 - 没有 Authorization header
curl -X POST http://43.163.101.37:3000/api/v1/mpc/session \
  --header 'content-type: application/json' \
  --data-raw '{"session_type":"keygen","parties":[0,1,2],"threshold":2}'

# ✅ 正确的请求 - 包含 Authorization header
curl -X POST http://43.163.101.37:3000/api/v1/mpc/session \
  --header 'Authorization: Bearer eyJ0...' \
  --header 'content-type: application/json' \
  --data-raw '{"session_type":"keygen","parties":[0,1,2],"threshold":2}'
```

### 问题 2: Flutter DioClient 拦截器的异步问题

**文件：** `mobile/lib/network/dio_client.dart`

```dart
InterceptorsWrapper(
  onRequest: (options, handler) async {
    // ⚠️ 问题：这是异步操作
    String? token = await SecureStorage.getToken();
    if (token != null) {
      options.headers["Authorization"] = "Bearer $token";
    }
    return handler.next(options);
  },
)
```

**可能的问题：**

1. **异步竞态条件** — Token 还没保存完成，MPC 请求就已发送
2. **SecureStorage 初始化延迟** — SecureStorage 可能还没初始化
3. **DioClient 单例初始化时序** — DioClient 在 token 保存前就被初始化了

### 问题 3: Onboarding 流程时序

**文件：** `mobile/lib/onboarding/onboarding_flow.dart`

```dart
// Step 1: 注册设备
authResult = await AuthApi.register(deviceId: deviceId);

// ⚠️ 问题：延迟不足，token 可能还没保存完成
Future.delayed(const Duration(milliseconds: 400), () async {
  // Step 2: 立即发送 MPC 会话请求
  await mpcService.startKeygen();
});
```

我们已经修复为 600ms，但可能还需要进一步调整。

## ✅ 已实施的修复

### 1. Dio 拦截器改进
**文件：** `mobile/lib/network/dio_client.dart`

```dart
// ✅ 添加异常处理和更好的日志
onRequest: (options, handler) async {
  try {
    String? token = await SecureStorage.getToken();
    if (token != null && token.isNotEmpty) {
      options.headers["Authorization"] = "Bearer $token";
      print("✅ [DioClient] Token added: ${token.substring(0, 30)}...");
    } else {
      print("⚠️  [DioClient] No token found in SecureStorage");
    }
  } catch (e) {
    print("❌ [DioClient] Error reading token: $e");
  }
  return handler.next(options);
}
```

### 2. Onboarding 延迟调整
**文件：** `mobile/lib/onboarding/onboarding_flow.dart`

```dart
// ✅ 注册后添加延迟确保 token 保存
await AuthApi.register(deviceId: deviceId);
await Future.delayed(const Duration(milliseconds: 200));

// ✅ MPC 请求延迟从 400ms 增加到 600ms
Future.delayed(const Duration(milliseconds: 600), () async {
  await mpcService.startKeygen();
});
```

### 3. Auth API 日志改进
**文件：** `mobile/lib/api/auth_api.dart`

```dart
if (token != null) {
  await SecureStorage.saveToken(token);
  print("✅ Token saved to SecureStorage");
} else {
  print("❌ Token is null in response");
}
```

## 🧪 验证方法

### 方法 1: 手动 curl 测试

运行测试脚本：
```bash
cd /Users/jingle/cat/cowallet
chmod +x test-auth-flow.sh
./test-auth-flow.sh
```

这将显示正确的 curl 命令和完整的请求/响应流程。

### 方法 2: Flutter 调试工具

从 `auth_debug_page.dart` 进行以下步骤：

1. 点击 **📝 测试注册** → 应该显示 `✅ Token 保存成功！`
2. 点击 **🔐 测试获取会话** → 应该返回 HTTP 200

### 方法 3: 查看 Flutter 日志

```bash
flutter logs
```

应该看到：
```
✅ [DioClient] Token added: eyJ0eXAiOiJKV1QiLCJhbGc...
✅ Token saved to SecureStorage
```

## 📋 完整的认证流程

```
1. 用户点击"注册"
   ↓
2. AuthApi.register(deviceId) 调用
   ↓
3. 发送 POST /api/v1/auth/register
   ↓
4. 后端返回 {"token": "...", "user_id": "..."}
   ↓
5. ✅ AuthApi 存储 token 到 SecureStorage
   ↓
6. ⏳ 等待 200ms 确保存储完成
   ↓
7. ⏳ 等待 600ms 确保 DioClient 准备好
   ↓
8. MpcWalletService.startKeygen() 调用
   ↓
9. 发送 POST /api/v1/mpc/session
   ↓
10. Dio 拦截器 onRequest 执行：
    - 读取 SecureStorage 中的 token
    - 添加 Authorization: Bearer <token> header
   ↓
11. 后端收到带有 Authorization header 的请求
   ↓
12. ✅ 返回 200 与 MPC 会话信息
```

## 🔧 如果问题仍未解决

### 检查清单

- [ ] Flutter 代码已更新到最新版本
- [ ] 已运行 `flutter clean && flutter pub get`
- [ ] 已重新编译应用
- [ ] FlutterSecureStorage 插件已正确配置
- [ ] Android/iOS 平台依赖已更新

### 额外的诊断步骤

1. **检查 SecureStorage 工作状态**
   ```dart
   await SecureStorage.save("test", "value");
   String? value = await SecureStorage.get("test");
   print("SecureStorage test: $value"); // 应该是 "value"
   ```

2. **手动测试 DioClient**
   ```dart
   final result = await DioClient.get("/health");
   print("Health check: $result.isSuccess"); // 应该是 true
   ```

3. **检查网络连接**
   ```bash
   curl -I http://43.163.101.37:3000/health
   # 应该返回 HTTP/1.1 200 OK
   ```

## 📞 获取更多帮助

如果问题仍未解决，请提供：

1. 完整的 Flutter 日志输出（`flutter logs`）
2. `test-auth-flow.sh` 的输出结果
3. AuthDebugPage 的测试结果截图
4. 后端服务器日志（`docker compose logs api-server`）

