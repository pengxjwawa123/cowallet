# Flutter 客户端认证调试指南

## 🔍 问题诊断

从您的日志看，问题是：
1. ✅ **注册请求成功** — 获取到了 token
2. ❌ **MPC 会话请求失败** — 返回 401，说明 Authorization header 没被发送

这表示 **token 没有被正确保存或读取**。

## 🛠️ 修复步骤

### Step 1: 更新 Flutter 代码

以下文件已修改：

**1. `mobile/lib/onboarding/onboarding_flow.dart`**
- ✅ 添加了延迟，确保 token 保存完成
- ✅ 从 400ms 改为 600ms，给更多时间给 SecureStorage

**2. `mobile/lib/network/dio_client.dart`**
- ✅ 添加了 debug 日志
- ✅ 显示 token 是否被成功添加到 header

**3. `mobile/lib/api/auth_api.dart`**
- ✅ 添加了详细的日志
- ✅ 确保 token 被正确保存

### Step 2: 使用调试工具

新增了 `mobile/lib/debug/auth_debug_page.dart` 来测试认证流程。

**在您的 main.dart 或路由中添加：**

```dart
import 'debug/auth_debug_page.dart';

// 在开发环境添加调试路由
routes: {
  '/debug/auth': (context) => const AuthDebugPage(),
  // ... 其他路由
}
```

或者直接在 `main.dart` 的首屏添加：

```dart
@override
Widget build(BuildContext context) {
  return MaterialApp(
    home: Scaffold(
      body: Column(
        children: [
          ElevatedButton(
            onPressed: () {
              Navigator.push(context, MaterialPageRoute(
                builder: (context) => const AuthDebugPage(),
              ));
            },
            child: const Text('打开认证调试工具'),
          ),
          // ... 主页内容
        ],
      ),
    ),
  );
}
```

### Step 3: 运行调试工具

1. 打开应用并导航到调试页面
2. 点击 **📝 测试注册** 按钮
3. 查看日志输出：
   - 应该看到 ✅ 注册成功
   - 应该看到 ✅ Token 保存成功
   - 应该看到 ✅ User ID 保存成功

4. 点击 **🔐 测试获取会话** 按钮
   - 应该返回 ✅ 获取会话成功
   - 如果返回 ❌ 401，说明 token 没被发送

### Step 4: 查看 Flutter 日志

运行应用时查看 Flutter 日志：

```bash
flutter logs
```

您应该看到类似的输出：

```
✅ Token added to Authorization header
✅ Token saved to SecureStorage
📝 AuthApi.register response: token=eyJ0eXAi..., userId=550e8400...
```

## 🔧 常见问题和解决方案

### 问题1: Token 为 null
**症状：** 日志显示 "Token is null in response"

**原因：** 后端返回格式不匹配

**解决方案：**
```dart
// 检查后端实际返回的 JSON 格式
// 使用 curl 测试：
curl -X POST http://43.163.101.37:3000/api/v1/auth/register \
  -H "Content-Type: application/json" \
  -d '{"device_id":"test"}'

// 查看返回的格式是否是：
// {"token": "...", "user_id": "..."}
```

### 问题2: Authorization header 没被添加
**症状：** 日志显示 "⚠️  Token is null or empty"

**原因：** SecureStorage 读取失败

**解决方案：**
```dart
// 检查 SecureStorage 是否正常工作
String? token = await SecureStorage.getToken();
print("Token: $token");

// 如果为 null，尝试手动保存和读取
await SecureStorage.saveToken("test-token");
String? savedToken = await SecureStorage.getToken();
print("Saved token: $savedToken");
```

### 问题3: flutter_secure_storage 权限问题
**症状：** SecureStorage 操作成功但数据丢失

**原因：** 可能是 Android 权限问题

**解决方案 (Android)：**
```xml
<!-- android/app/src/main/AndroidManifest.xml -->
<uses-permission android:name="android.permission.USE_CREDENTIALS" />
<uses-permission android:name="android.permission.GET_ACCOUNTS" />
```

**解决方案 (iOS)：**
```
在 Runner.xcodeproj 中设置 Keychain sharing
```

## 📊 期望的日志输出

### 成功的注册流程：

```
• 📱 设备 ID: device-123
• 🔄 开始测试注册流程...
• ⏳ 调用 AuthApi.register()...
• ✅ 注册成功
•    Token: eyJ0eXAiOiJKV1QiLCJhbGc...
•    User ID: 550e8400-e29b-41d4-a716
• ✅ 验证存储
•    已保存 Token: eyJ0eXAiOiJKV1QiLCJhbGc...
•    已保存 User ID: 550e8400-e29b-41d4-a716
• ✅ Token 保存成功！
```

### 成功的会话验证：

```
• 🔄 开始测试获取会话...
• ✅ 找到已保存的 token: eyJ0eXAiOiJKV1QiLCJhbGc...
• ⏳ 调用 AuthApi.getSessionInfo()...
• ✅ 获取会话成功
•    响应: {user_id: 550e8400..., device_id: device-123, expires_at: 1777629927}
```

## 🚀 后续步骤

1. **收集调试日志** — 运行上述步骤并截图或复制日志
2. **提交问题** — 如果仍有问题，提供完整的日志输出
3. **部署修复** — 确认没问题后，将修改推送到服务器

## ✅ 验证清单

- [ ] DioClient 初始化时没有错误
- [ ] SecureStorage 能正常保存 token
- [ ] onRequest 拦截器能读取 token
- [ ] Authorization header 正确格式：`Bearer <token>`
- [ ] 后续请求能成功通过认证（HTTP 200 而不是 401）

