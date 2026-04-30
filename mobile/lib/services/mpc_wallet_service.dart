import 'dart:convert';
import '../api/mpc_api.dart';
import '../utils/secure_storage.dart';

/// MPC 门限签名钱包服务
/// 实现 2-of-3 门限签名密钥生成
/// - 分片 0: 本地设备 (Secure Enclave)
/// - 分片 1: 后端服务
/// - 分片 2: 助记词备份 (用户保管)
class MpcWalletService {
  String? _currentSessionId;
  final int _currentParty = 0; // 设备端始终是 party 0

  /// 开始 MPC 密钥生成会话
  Future<String> startKeygen() async {
    final result = await MpcApi.createSession(
      sessionType: 'keygen',
      parties: [0, 1, 2], // device, server, backup
      threshold: 2,
    );

    if (result.isSuccess && result.data != null) {
      _currentSessionId = result.data!['session_id'] as String?;
      return _currentSessionId!;
    }

    throw Exception('Failed to start MPC session: ${result.errorMessage}');
  }

  /// 执行密钥生成协议 (简化版 - 实际需要完整的 MPC 库)
  /// 这是一个模拟实现，展示了协议的结构
  Future<Map<String, dynamic>> runKeygenProtocol(String sessionId) async {
    // 实际的 MPC 实现会使用:
    // - https://github.com/ZenGo-X/multi-party-ecdsa (Rust)
    // - https://github.com/taironas/mpc-ecdsa-dart (Dart 绑定)
    //
    // 这里模拟三轮消息交换的过程:
    // 1. Round 1: 承诺与证明
    // 2. Round 2:  Shamir 秘密共享
    // 3. Round 3: 范围证明和一致性验证

    const maxRounds = 3;

    for (var round = 1; round <= maxRounds; round++) {
      // 模拟生成本轮消息
      final messagePayload = _generateRoundMessage(round);

      // 发送给其他参与方 (party 1 = server, party 2 = backup)
      await MpcApi.sendMessage(
        sessionId: sessionId,
        fromParty: _currentParty,
        toParty: 1, // 发给服务端
        round: round,
        payload: messagePayload,
      );

      // 接收来自服务端的消息
      final serverMessages = await MpcApi.receiveMessages(sessionId);

      if (!serverMessages.isSuccess) {
        throw Exception('Failed to receive MPC messages for round $round');
      }

      // 处理收到的消息...
      await Future.delayed(const Duration(milliseconds: 300));
    }

    // 模拟生成本地密钥分片
    final keyShare = _generateLocalKeyShare();

    // 安全存储本地分片
    await SecureStorage.save('mpc_key_share_0', keyShare['private']!);
    await SecureStorage.save('mpc_public_key', keyShare['public']!);
    await SecureStorage.save('mpc_chain_code', keyShare['chainCode']!);

    return keyShare;
  }

  /// 生成本轮 MPC 消息 (模拟)
  List<int> _generateRoundMessage(int round) {
    final message = {
      'round': round,
      'party': _currentParty,
      'timestamp': DateTime.now().millisecondsSinceEpoch,
      // 实际会包含: paillier 公钥, Pedersen 承诺, 零知识证明等
      'data': 'round_${round}_payload',
    };
    return utf8.encode(jsonEncode(message));
  }

  /// 生成本地密钥分片 (模拟)
  Map<String, String> _generateLocalKeyShare() {
    // 实际的 MPC 实现会在这里计算密钥分片
    // 这里使用模拟值，后续应接入真实的 MPC 库
    final timestamp = DateTime.now().millisecondsSinceEpoch;
    return {
      'private': 'simulated_key_share_0_$timestamp',
      'public': '02' * 32 + '_simulated_pubkey',
      'chainCode': '00' * 32,
      'address': '0x' + '1' * 40, // 模拟地址
    };
  }

  /// 获取钱包地址
  Future<String?> getWalletAddress() async {
    return await SecureStorage.get('mpc_public_key');
  }

  /// 检查是否已有 MPC 钱包
  Future<bool> hasMpcWallet() async {
    final share = await SecureStorage.get('mpc_key_share_0');
    return share != null && share.isNotEmpty;
  }

  /// 清除 MPC 钱包数据
  Future<void> clear() async {
    await SecureStorage.delete('mpc_key_share_0');
    await SecureStorage.delete('mpc_public_key');
    await SecureStorage.delete('mpc_chain_code');
  }

  /// 获取当前会话ID
  String? get currentSessionId => _currentSessionId;
}
