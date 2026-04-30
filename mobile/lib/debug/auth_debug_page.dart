import 'package:flutter/material.dart';
import '../api/auth_api.dart';
import '../utils/secure_storage.dart';
import '../utils/device_id.dart';

/// 认证流程调试页面
/// 用于测试和验证 token 保存、读取、发送流程
class AuthDebugPage extends StatefulWidget {
  const AuthDebugPage({super.key});

  @override
  State<AuthDebugPage> createState() => _AuthDebugPageState();
}

class _AuthDebugPageState extends State<AuthDebugPage> {
  String _log = "";
  String? _currentToken;
  String? _currentUserId;
  String? _deviceId;

  @override
  void initState() {
    super.initState();
    _loadCurrentState();
  }

  Future<void> _loadCurrentState() async {
    final token = await SecureStorage.getToken();
    final userId = await SecureStorage.getUserId();
    final deviceId = await DeviceIdGenerator.getOrGenerate();

    setState(() {
      _currentToken = token;
      _currentUserId = userId;
      _deviceId = deviceId;
      _addLog("📱 设备 ID: $deviceId");
      _addLog("🔐 已保存的 Token: ${token?.substring(0, 30) ?? 'null'}...");
      _addLog("👤 已保存的 User ID: $userId");
    });
  }

  void _addLog(String message) {
    setState(() {
      _log += "• $message\n";
    });
  }

  Future<void> _testRegister() async {
    _log = "";
    _addLog("🔄 开始测试注册流程...\n");

    try {
      final deviceId = _deviceId ?? await DeviceIdGenerator.getOrGenerate();
      _addLog("📱 使用设备 ID: $deviceId");

      _addLog("⏳ 调用 AuthApi.register()...");
      final result = await AuthApi.register(deviceId: deviceId);

      if (result.isSuccess) {
        _addLog("✅ 注册成功");
        _addLog("   Token: ${result.data?["token"]?.substring(0, 30)}...");
        _addLog("   User ID: ${result.data?["user_id"]}");

        // 检查 token 是否被保存
        await Future.delayed(const Duration(milliseconds: 300));
        final savedToken = await SecureStorage.getToken();
        final savedUserId = await SecureStorage.getUserId();

        _addLog("\n✅ 验证存储");
        _addLog("   已保存 Token: ${savedToken?.substring(0, 30) ?? "❌ null"}...");
        _addLog("   已保存 User ID: ${savedUserId ?? "❌ null"}");

        if (savedToken != null) {
          _addLog("\n✅ Token 保存成功！");
        } else {
          _addLog("\n❌ Token 保存失败！");
        }

        setState(() {
          _currentToken = savedToken;
          _currentUserId = savedUserId;
        });
      } else {
        _addLog("❌ 注册失败: ${result.errorMessage}");
      }
    } catch (e) {
      _addLog("❌ 错误: $e");
    }
  }

  Future<void> _testGetSession() async {
    _log = "";
    _addLog("🔄 开始测试获取会话...\n");

    try {
      // 先检查是否有 token
      final token = await SecureStorage.getToken();
      if (token == null) {
        _addLog("❌ 没有保存的 token，请先注册");
        return;
      }

      _addLog("✅ 找到已保存的 token: ${token.substring(0, 30)}...");

      _addLog("⏳ 调用 AuthApi.getSessionInfo()...");
      final result = await AuthApi.getSessionInfo();

      if (result.isSuccess) {
        _addLog("✅ 获取会话成功");
        _addLog("   响应: ${result.data}");
      } else {
        _addLog("❌ 获取会话失败 (${result.errorCode}): ${result.errorMessage}");
      }
    } catch (e) {
      _addLog("❌ 错误: $e");
    }
  }

  Future<void> _clearStorage() async {
    await SecureStorage.clearAll();
    _addLog("🗑️  已清除所有存储的数据");
    await _loadCurrentState();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('认证调试工具'),
        backgroundColor: Colors.blue[900],
      ),
      body: Column(
        children: [
          // 状态显示
          Container(
            padding: const EdgeInsets.all(16),
            color: Colors.grey[100],
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                const Text('📊 当前状态:', style: TextStyle(fontWeight: FontWeight.bold)),
                const SizedBox(height: 8),
                Text('设备 ID: $_deviceId'),
                Text('Token: ${_currentToken?.substring(0, 30) ?? 'null'}...'),
                Text('User ID: $_currentUserId'),
              ],
            ),
          ),
          // 按钮
          Padding(
            padding: const EdgeInsets.all(16),
            child: Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                ElevatedButton(
                  onPressed: _testRegister,
                  style: ElevatedButton.styleFrom(backgroundColor: Colors.green),
                  child: const Text('📝 测试注册'),
                ),
                ElevatedButton(
                  onPressed: _testGetSession,
                  style: ElevatedButton.styleFrom(backgroundColor: Colors.blue),
                  child: const Text('🔐 测试获取会话'),
                ),
                ElevatedButton(
                  onPressed: _clearStorage,
                  style: ElevatedButton.styleFrom(backgroundColor: Colors.red),
                  child: const Text('🗑️  清除存储'),
                ),
              ],
            ),
          ),
          // 日志输出
          Expanded(
            child: Container(
              margin: const EdgeInsets.all(16),
              padding: const EdgeInsets.all(12),
              decoration: BoxDecoration(
                border: Border.all(color: Colors.grey),
                borderRadius: BorderRadius.circular(8),
                color: Colors.grey[50],
              ),
              child: SingleChildScrollView(
                child: Text(
                  _log.isEmpty ? "准备好了，点击上方按钮开始测试" : _log,
                  style: const TextStyle(
                    fontFamily: 'monospace',
                    fontSize: 12,
                  ),
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}
