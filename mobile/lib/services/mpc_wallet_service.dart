import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';
import '../api/mpc_api.dart';
import '../bridge/mpc_bridge.dart';
import '../network/mpc_websocket.dart';
import '../platform/cloud_backup.dart';
import '../platform/secure_hardware.dart';
import '../utils/secure_storage.dart';
import 'backup_shard_service.dart';
import 'key_health_service.dart';
import 'wallet_service.dart';
import 'mpc_session_store.dart';

/// MPC 门限签名钱包服务
/// 实现 2-of-3 门限签名密钥生成、签名、密钥轮转、预签名
/// - Party 0: 本地设备 (Secure Enclave / StrongBox)
/// - Party 1: 后端服务 (自动参与协议)
/// - Party 2: 备份分片 (iCloud Keychain / Google Cloud Backup / 用户离线保管)
class MpcWalletService implements WalletService {
  String? _currentSessionId;
  BackupResult? _lastBackupResult;
  List<int>? _lastBackupShard;
  int _lastMessageId = 0;
  static const int _deviceParty = 0;
  static const int _serverParty = 1;
  static const int _backupParty = 2;
  static const Duration _wsTimeout = Duration(seconds: 5);

  /// 执行完整的 DKG 密钥生成协议
  /// [walletId] 可选，用于多钱包场景
  Future<WalletInfo> runDkg({String? walletId}) async {
    final sessionResult = await MpcApi.createSession(
      sessionType: 'keygen',
      parties: [_deviceParty, _serverParty],
      threshold: 2,
      walletId: walletId,
    );

    if (!sessionResult.isSuccess || sessionResult.data == null) {
      throw MpcException('Failed to create DKG session: ${sessionResult.errorMessage}');
    }

    final sessionId = sessionResult.data!['session_id'] as String;
    _currentSessionId = sessionId;
    _lastMessageId = 0;

    final ws = MpcWebSocket(sessionId: sessionId, partyIndex: _deviceParty);
    try {
      await ws.connect();

      // Subscribe to messages immediately after connect to capture catch-up messages
      final serverMessages = <MpcMessage>[];
      final messagesReady = Completer<List<MpcMessage>>();
      final subscription = ws.messages.listen((msg) {
        if (msg.fromParty == _serverParty) {
          serverMessages.add(msg);
          if (serverMessages.length >= 2 && !messagesReady.isCompleted) {
            messagesReady.complete(serverMessages);
          }
        }
      });

      final localSessionId = await MpcBridge.dkgSessionNew(_deviceParty);

      // Save initial session state for recovery
      await MpcSessionStore.saveSession(MpcSessionState(
        sessionId: localSessionId,
        remoteSessionId: sessionId,
        sessionType: 'keygen',
        currentRound: 0,
        createdAt: DateTime.now(),
      ));

      final round1Json = await MpcBridge.dkgGenerateRound1(localSessionId);
      // Server expects the raw payload bytes from the ProtocolMessage, not the full JSON.
      // Extract the "payload" field (array of ints) from the JSON.
      final round1Msg = jsonDecode(round1Json) as Map<String, dynamic>;
      final round1Payload = List<int>.from(round1Msg['payload'] as List);

      // Send Round 1 via HTTP (reliable delivery)
      await MpcApi.sendMessage(
        sessionId: sessionId,
        fromParty: _deviceParty,
        toParty: _serverParty,
        round: 1,
        payload: round1Payload,
      );

      // Update progress
      await MpcSessionStore.updateCurrentRound(1);

      // Wait for server's Round 1 + Round 2 (listener started before send)
      final allServerMessages = await messagesReady.future.timeout(
        _wsTimeout,
        onTimeout: () async {
          await subscription.cancel();
          // Fallback to HTTP polling
          if (_currentSessionId != null) {
            return await _pollMessagesFallback(
              sessionId: _currentSessionId!,
              party: _deviceParty,
              expectedCount: 2,
            );
          }
          throw MpcException('Timeout waiting for server response via WebSocket');
        },
      );
      await subscription.cancel();

      // Server returns raw payload bytes; wrap them into ProtocolMessage JSON for the FFI.
      final serverRound1Msgs = allServerMessages
          .where((m) => m.round == 1)
          .map((m) => _wrapAsProtocolMessage(sessionId, m))
          .toList();

      await MpcBridge.dkgProcessRound1(localSessionId, serverRound1Msgs);

      final round2Msgs = await MpcBridge.dkgGenerateRound2(localSessionId);
      for (final msgJson in round2Msgs) {
        final msg = jsonDecode(msgJson) as Map<String, dynamic>;
        final to = msg['to'] as int;
        if (to == _serverParty) {
          final round2Payload = List<int>.from(msg['payload'] as List);
          // Send Round 2 via HTTP (reliable delivery)
          await MpcApi.sendMessage(
            sessionId: sessionId,
            fromParty: _deviceParty,
            toParty: _serverParty,
            round: 2,
            payload: round2Payload,
          );
        }
      }

      // Update progress
      await MpcSessionStore.updateCurrentRound(2);

      // Process server's Round 2
      final serverRound2Msgs = allServerMessages
          .where((m) => m.round == 2 && m.fromParty == _serverParty)
          .map((m) => _wrapAsProtocolMessage(sessionId, m))
          .toList();

      if (serverRound2Msgs.isNotEmpty) {
        await MpcBridge.dkgProcessRound2(localSessionId, serverRound2Msgs);
      }

      final walletInfo = await MpcBridge.dkgFinalize(localSessionId);

      // Derive backup shard (device + server combined) and keep in memory.
      // The UI will prompt the user to choose a storage method.
      try {
        _lastBackupShard = await _deriveBackupShard(localSessionId);
        print('[MpcWalletService] Backup shard derived successfully (${_lastBackupShard!.length} bytes)');
      } catch (e) {
        print('[MpcWalletService] Backup shard derivation skipped: $e');
      }

      await SecureStorage.save('mpc_address', walletInfo.address);
      await SecureStorage.save('mpc_session_id', sessionId);

      // Fresh DKG: clear stale recovery commitment so verification uses Lagrange
      await SecureStorage.delete('mpc_server_commitment');

      // Persist device shard to hardware-backed storage and public key to secure storage
      final deviceShardBytes = await MpcBridge.exportDeviceShard();
      await SecureHardware.storeDeviceShard(Uint8List.fromList(deviceShardBytes));
      final pubKeyHex = walletInfo.publicKey.map((b) => b.toRadixString(16).padLeft(2, '0')).join();
      await SecureStorage.save('mpc_public_key', pubKeyHex);

      // Clear session state on success
      await MpcSessionStore.clearSession();

      return walletInfo;
    } catch (e) {
      // Save state on error for potential recovery
      print('[MpcWalletService] DKG error: $e');
      throw MpcSessionInterruptedException(
        'DKG session interrupted: $e',
        sessionState: await MpcSessionStore.loadSession(),
      );
    } finally {
      await ws.disconnect();
    }
  }

  /// 按需加载设备分片到 Rust 内存（签名前调用）
  /// Public so MpcSessionManager can call it during sign recovery.
  Future<void> ensureShardLoaded() async {
    final shardBytes = await SecureHardware.loadDeviceShard();

    if (shardBytes == null || shardBytes.isEmpty) {
      throw MpcException('Device shard not found in secure hardware');
    }

    final pubKeyHex = await SecureStorage.get('mpc_public_key');
    if (pubKeyHex == null || pubKeyHex.isEmpty) {
      throw MpcException('Public key not found');
    }

    final publicKey = List<int>.generate(
      pubKeyHex.length ~/ 2,
      (i) => int.parse(pubKeyHex.substring(i * 2, i * 2 + 2), radix: 16),
    );

    await MpcBridge.importDeviceShard(
      shardBytes: shardBytes.toList(),
      publicKey: publicKey,
    );
  }

  /// 执行分布式签名协议 (2-party ECDSA, 私钥从未被重组)
  /// [msgHash] 32字节消息哈希
  /// [walletId] 可选，指定使用哪个钱包的密钥分片签名
  Future<List<int>> runSign(List<int> msgHash, {String? walletId}) async {
    if (msgHash.length != 32) {
      throw MpcException('Message hash must be exactly 32 bytes');
    }

    await ensureShardLoaded();

    final sessionResult = await MpcApi.createSession(
      sessionType: 'sign',
      parties: [_deviceParty, _serverParty],
      threshold: 2,
      walletId: walletId,
    );

    if (!sessionResult.isSuccess || sessionResult.data == null) {
      throw MpcException('Failed to create sign session: ${sessionResult.errorMessage}');
    }

    final remoteSessionId = sessionResult.data!['session_id'] as String;
    _currentSessionId = remoteSessionId;
    _lastMessageId = 0;

    final ws = MpcWebSocket(sessionId: remoteSessionId, partyIndex: _deviceParty);
    try {
      await ws.connect();

      final round1 = await MpcBridge.signGenerateRound1(msgHash);
      final localSessionId = round1.sessionId;

      // Save initial session state for recovery
      await MpcSessionStore.saveSession(MpcSessionState(
        sessionId: localSessionId,
        remoteSessionId: remoteSessionId,
        sessionType: 'sign',
        currentRound: 0,
        createdAt: DateTime.now(),
        metadata: {'msg_hash': msgHash},
      ));

      // Send Round 1 + msg_hash via HTTP (reliable delivery)
      final round1WithHash = [...round1.payload, ...msgHash];
      await MpcApi.sendMessage(
        sessionId: remoteSessionId,
        fromParty: _deviceParty,
        toParty: _serverParty,
        round: 1,
        payload: round1WithHash,
      );

      await MpcSessionStore.updateCurrentRound(1);

      // Wait for server's Round 1 (R_1)
      final serverR1 = await _waitForMessages(ws, expectedCount: 1);
      final serverR1Payload = serverR1.first.payload;

      // Sync _lastMessageId so Round 2 fallback skips Round 1 messages
      await _syncLastMessageId(remoteSessionId);

      // Process R_1 and generate Round 2
      final round2Payload = await MpcBridge.signProcessRound1AndGenerateRound2(
        localSessionId,
        serverR1Payload,
      );

      // Send DeviceContribution via HTTP (reliable delivery)
      await MpcApi.sendMessage(
        sessionId: remoteSessionId,
        fromParty: _deviceParty,
        toParty: _serverParty,
        round: 2,
        payload: round2Payload,
      );

      await MpcSessionStore.updateCurrentRound(2);

      // Wait for server's signature
      final serverR2 = await _waitForMessages(ws, expectedCount: 1);
      final serverR2Payload = serverR2.first.payload;

      final signature = await MpcBridge.signProcessRound2(
        localSessionId,
        serverR2Payload,
      );

      if (signature.length != 65) {
        throw MpcException('Invalid signature length: ${signature.length}');
      }

      // Clear session state on success
      await MpcSessionStore.clearSession();

      // Record key usage for health tracking
      final health = KeyHealthService();
      health.recordPhoneKeyUsage();
      health.recordServerKeyUsage();

      return signature;
    } catch (e) {
      print('[MpcWalletService] Sign error: $e');
      throw MpcSessionInterruptedException(
        'Sign session interrupted: $e',
        sessionState: await MpcSessionStore.loadSession(),
      );
    } finally {
      await ws.disconnect();
    }
  }

  /// 执行密钥轮转协议 (Reshare)
  /// 刷新密钥分片，旧分片失效，公钥不变
  /// [walletId] 可选，指定要轮转的钱包
  Future<WalletInfo> runReshare({String? walletId}) async {
    final sessionResult = await MpcApi.createSession(
      sessionType: 'reshare',
      parties: [_deviceParty, _serverParty],
      threshold: 2,
      walletId: walletId,
    );

    if (!sessionResult.isSuccess || sessionResult.data == null) {
      throw MpcException('Failed to create reshare session: ${sessionResult.errorMessage}');
    }

    final remoteSessionId = sessionResult.data!['session_id'] as String;
    _currentSessionId = remoteSessionId;

    final ws = MpcWebSocket(sessionId: remoteSessionId, partyIndex: _deviceParty);
    try {
      await ws.connect();

      // Initialize local reshare session
      final localSessionId = await MpcBridge.reshareSessionNew(_deviceParty);

      // Save initial session state for recovery
      await MpcSessionStore.saveSession(MpcSessionState(
        sessionId: localSessionId,
        remoteSessionId: remoteSessionId,
        sessionType: 'reshare',
        currentRound: 0,
        createdAt: DateTime.now(),
      ));

      // Generate Round 1 (new polynomial evaluations)
      final round1Messages = await MpcBridge.reshareGenerateRound1(localSessionId);

      // Send evaluations addressed to server via WebSocket
      for (final msgJson in round1Messages) {
        final msg = jsonDecode(msgJson) as Map<String, dynamic>;
        final to = msg['to'] as int;
        if (to == _serverParty) {
          ws.sendRaw(
            toParty: _serverParty,
            round: 1,
            payload: utf8.encode(msgJson),
          );
        }
      }

      await MpcSessionStore.updateCurrentRound(1);

      // Wait for server's reshare Round 1 messages (its evaluations for us)
      final serverMessages = await _waitForMessages(ws, expectedCount: 1);
      final serverMsgsJson = serverMessages
          .map((m) => utf8.decode(m.payload))
          .toList();

      // Process server's evaluations and compute new share
      await MpcBridge.reshareProcessRound1(localSessionId, serverMsgsJson);

      // Finalize: new shard replaces old in memory
      final walletInfo = await MpcBridge.reshareFinalize(localSessionId);

      // Update stored address (should be unchanged)
      await SecureStorage.save('mpc_address', walletInfo.address);

      // Clear session state on success
      await MpcSessionStore.clearSession();

      return walletInfo;
    } catch (e) {
      print('[MpcWalletService] Reshare error: $e');
      throw MpcSessionInterruptedException(
        'Reshare session interrupted: $e',
        sessionState: await MpcSessionStore.loadSession(),
      );
    } finally {
      await ws.disconnect();
    }
  }

  /// 执行预签名协议 (Presign)
  /// 预计算签名材料，后续签名可瞬间完成
  /// [walletId] 钱包ID
  /// [count] 要生成的预签名数量
  Future<int> runPresign({required String walletId, int count = 5}) async {
    int generated = 0;

    for (int i = 0; i < count; i++) {
      final sessionResult = await MpcApi.createSession(
        sessionType: 'presign',
        parties: [_deviceParty, _serverParty],
        threshold: 2,
        walletId: walletId,
      );

      if (!sessionResult.isSuccess || sessionResult.data == null) {
        break;
      }

      final remoteSessionId = sessionResult.data!['session_id'] as String;

      final ws = MpcWebSocket(sessionId: remoteSessionId, partyIndex: _deviceParty);
      try {
        await ws.connect();

        // Generate presign Round 1
        final round1 = await MpcBridge.presignGenerateRound1();
        final localSessionId = round1.sessionId;

        // Send Round 1 to server
        ws.sendRaw(toParty: _serverParty, round: 1, payload: round1.payload);

        // Wait for server's Round 1
        final serverR1 = await _waitForMessages(ws, expectedCount: 1);

        // Process and generate Round 2
        final round2Payload = await MpcBridge.presignProcessRound1AndGenerateRound2(
          localSessionId,
          serverR1.first.payload,
        );

        // Send Round 2
        ws.sendRaw(toParty: _serverParty, round: 2, payload: round2Payload);

        // Finalize presignature
        await MpcBridge.presignFinalize(localSessionId);
        generated++;
      } finally {
        await ws.disconnect();
      }
    }

    return generated;
  }

  /// 提取并存储备份分片
  /// 计算完整备份分片 (f_device(3) + f_server(3))
  /// 返回 32 字节标量，不自动存储。UI 层负责让用户选择存储方式。
  Future<List<int>> _deriveBackupShard(String localSessionId) async {
    // Step 1: Compute device's contribution to backup shard (f_device(3))
    final deviceContribution = await MpcBridge.dkgDeriveBackupShare(
      localSessionId,
      backupPartyIndex: _backupParty,
    );

    if (deviceContribution.length != 32) {
      throw MpcException(
        'Invalid device backup contribution length: ${deviceContribution.length} bytes (expected 32)'
      );
    }

    // Step 2: Fetch server's contribution (f_server(3)) from API
    if (_currentSessionId == null) {
      throw MpcException('No active session ID for fetching server backup contribution');
    }

    final serverResult = await MpcApi.getBackupContribution(_currentSessionId!);
    if (!serverResult.isSuccess || serverResult.data == null) {
      throw MpcException(
        'Failed to fetch server backup contribution: ${serverResult.errorMessage}'
      );
    }

    final serverContribution = serverResult.data!;
    if (serverContribution.length != 32) {
      throw MpcException(
        'Invalid server backup contribution length: ${serverContribution.length} bytes (expected 32)'
      );
    }

    // Step 3: Combine both contributions via modular scalar addition
    final combinedBackupShard = await MpcBridge.combineBackupShares(
      deviceShare: deviceContribution,
      serverShare: serverContribution,
    );

    if (combinedBackupShard.length != 32) {
      throw MpcException(
        'Invalid combined backup shard length: ${combinedBackupShard.length} bytes (expected 32)'
      );
    }

    return combinedBackupShard;
  }

  /// 获取 DKG 后计算好的备份分片（内存中，未存储）
  /// UI 层应调用此方法获取数据，然后让用户选择存储方式
  List<int>? get lastBackupShard => _lastBackupShard;

  /// 用户选择存储方式后调用此方法
  Future<BackupResult> storeBackupShard(List<int> shardBytes, {required bool useCloud}) async {
    final backupService = BackupShardService(PlatformCloudBackup());
    final addr = await getAddress();
    if (addr.isNotEmpty) {
      backupService.setWalletAddress(addr);
    }
    _lastBackupResult = await backupService.storeBackupShard(shardBytes, useCloud: useCloud);
    _lastBackupShard = null;
    return _lastBackupResult!;
  }

  /// 获取上次 DKG 的备份结果
  BackupResult? get lastBackupResult => _lastBackupResult;

  @override
  Future<List<int>> sign(List<int> msgHash) async {
    return await runSign(msgHash);
  }

  @override
  Future<SignResult> signWithSession(List<int> msgHash) async {
    final signature = await runSign(msgHash);
    return SignResult(signature: signature, sessionId: _currentSessionId);
  }

  @override
  Future<String> getAddress() async {
    final addr = await SecureStorage.get('mpc_address');
    if (addr == null || addr.isEmpty) {
      throw StateError('No MPC wallet found');
    }
    return addr;
  }

  @override
  Future<bool> hasWallet() async {
    final addr = await SecureStorage.get('mpc_address');
    return addr != null && addr.isNotEmpty;
  }

  @override
  Future<void> deleteWallet() async {
    await SecureStorage.delete('mpc_address');
    await SecureStorage.delete('mpc_session_id');
    await SecureStorage.delete('mpc_key_share_0');
    await SecureStorage.delete('mpc_public_key');
    await SecureStorage.delete('mpc_chain_code');
  }

  /// 通过 WebSocket 流等待指定数量的服务器消息
  Future<List<MpcMessage>> _waitForMessages(
    MpcWebSocket ws, {
    required int expectedCount,
  }) async {
    final messages = <MpcMessage>[];
    final completer = Completer<List<MpcMessage>>();

    final subscription = ws.messages.listen((msg) {
      if (msg.fromParty == _serverParty) {
        messages.add(msg);
        if (messages.length >= expectedCount && !completer.isCompleted) {
          completer.complete(messages);
        }
      }
    });

    // Fallback timeout with HTTP polling
    final timer = Timer(_wsTimeout, () {
      if (!completer.isCompleted) {
        subscription.cancel();
        completer.completeError(
          MpcException('Timeout waiting for server response via WebSocket'),
        );
      }
    });

    try {
      final result = await completer.future;
      timer.cancel();
      await subscription.cancel();
      return result;
    } catch (e) {
      timer.cancel();
      await subscription.cancel();

      // Fallback to HTTP polling if WebSocket failed
      if (e is MpcException && _currentSessionId != null) {
        return await _pollMessagesFallback(
          sessionId: _currentSessionId!,
          party: _deviceParty,
          expectedCount: expectedCount,
          afterId: _lastMessageId,
        );
      }
      rethrow;
    }
  }

  /// Sync _lastMessageId by querying current max message ID from server.
  Future<void> _syncLastMessageId(String sessionId) async {
    try {
      final result = await MpcApi.receiveMessages(
        sessionId,
        party: _deviceParty,
        afterId: 0,
      );
      if (result.isSuccess && result.data != null) {
        for (final raw in result.data!) {
          final m = Map<String, dynamic>.from(raw as Map);
          final id = m['id'] as int;
          if (id > _lastMessageId) _lastMessageId = id;
        }
      }
    } catch (_) {}
  }

  /// HTTP 轮询回退（WebSocket 不可用时）
  Future<List<MpcMessage>> _pollMessagesFallback({
    required String sessionId,
    required int party,
    required int expectedCount,
    int afterId = 0,
  }) async {
    const pollInterval = Duration(seconds: 1);
    const pollTimeout = Duration(seconds: 10);
    final deadline = DateTime.now().add(pollTimeout);
    List<MpcMessage> allMessages = [];
    int lastId = afterId;

    while (DateTime.now().isBefore(deadline)) {
      final result = await MpcApi.receiveMessages(
        sessionId,
        party: party,
        afterId: lastId,
      );

      if (result.isSuccess && result.data != null) {
        for (final raw in result.data!) {
          final m = Map<String, dynamic>.from(raw as Map);
          if (m['from_party'] == _serverParty) {
            allMessages.add(MpcMessage(
              fromParty: m['from_party'] as int,
              toParty: m['to_party'] as int,
              round: m['round'] as int,
              payload: (m['payload'] as List<dynamic>).cast<int>(),
            ));
            final id = m['id'] as int;
            if (id > lastId) lastId = id;
            if (id > _lastMessageId) _lastMessageId = id;
          }
        }

        if (allMessages.length >= expectedCount) {
          return allMessages;
        }
      }

      await Future.delayed(pollInterval);
    }

    if (allMessages.isEmpty) {
      throw MpcException('Timeout waiting for server response (HTTP fallback)');
    }
    return allMessages;
  }

  /// 获取当前会话ID
  String? get currentSessionId => _currentSessionId;

  /// Wrap raw server payload bytes into a ProtocolMessage JSON string
  /// that the Rust FFI expects.
  String _wrapAsProtocolMessage(String sessionId, MpcMessage msg) {
    return jsonEncode({
      'session_id': sessionId,
      'from': msg.fromParty,
      'to': msg.toParty,
      'round': msg.round,
      'payload': msg.payload,
    });
  }
}
