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
  static const int _deviceParty = 0;
  static const int _serverParty = 1;
  static const int _backupParty = 2;
  static const Duration _wsTimeout = Duration(seconds: 30);

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

    final ws = MpcWebSocket(sessionId: sessionId, partyIndex: _deviceParty, disableAutoReconnect: true);
    try {
      await ws.connect();

      // Collect server messages from both WS and HTTP polling.
      // Server's Round 1 is already in DB (generated during createSession),
      // so we poll HTTP as primary source rather than relying on WS catch-up race.
      final serverMessages = <MpcMessage>[];
      final messagesReady = Completer<List<MpcMessage>>();
      final subscription = ws.messages.listen((msg) {
        if (msg.fromParty == _serverParty) {
          final alreadyHas = serverMessages.any(
            (m) => m.round == msg.round && m.fromParty == msg.fromParty,
          );
          if (!alreadyHas) {
            serverMessages.add(msg);
            if (serverMessages.length >= 2 && !messagesReady.isCompleted) {
              messagesReady.complete(serverMessages);
            }
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

      // Fetch server's Round 1 via HTTP immediately — it's guaranteed to be in DB
      // since createSession generates it synchronously before returning.
      final serverR1Poll = await MpcApi.receiveMessages(
        sessionId,
        party: _deviceParty,
      );
      if (serverR1Poll.isSuccess && serverR1Poll.data != null) {
        for (final raw in serverR1Poll.data!) {
          final m = Map<String, dynamic>.from(raw as Map);
          if (m['from_party'] == _serverParty) {
            serverMessages.add(MpcMessage(
              fromParty: m['from_party'] as int,
              toParty: m['to_party'] as int,
              round: m['round'] as int,
              payload: (m['payload'] as List<dynamic>).cast<int>(),
            ));
          }
        }
      }
      if (serverMessages.isEmpty) {
        throw MpcException('Server failed to generate DKG Round 1 — session may have failed on server');
      }

      final round1Json = await MpcBridge.dkgGenerateRound1(localSessionId);
      final round1Msg = jsonDecode(round1Json) as Map<String, dynamic>;
      final round1Payload = List<int>.from(round1Msg['payload'] as List);

      // Send Round 1 via HTTP (reliable delivery, triggers server Round 2)
      await MpcApi.sendMessage(
        sessionId: sessionId,
        fromParty: _deviceParty,
        toParty: _serverParty,
        round: 1,
        payload: round1Payload,
      );

      // Update progress
      await MpcSessionStore.updateCurrentRound(1);

      // Wait for server's Round 2 (triggered by our Round 1 send).
      // We already have Round 1, so we only need 1 more message.
      if (serverMessages.length < 2) {
        final allServerMessages = await messagesReady.future.timeout(
          _wsTimeout,
          onTimeout: () async {
            // Fallback to HTTP polling for Round 2
            final polled = await _pollMessagesFallback(
              sessionId: sessionId,
              party: _deviceParty,
              expectedCount: 2,
            );
            return polled;
          },
        );
        // Merge any polled messages we don't already have
        for (final msg in allServerMessages) {
          final alreadyHas = serverMessages.any(
            (m) => m.round == msg.round && m.fromParty == msg.fromParty,
          );
          if (!alreadyHas) serverMessages.add(msg);
        }
      }
      await subscription.cancel();
      final allServerMessages = serverMessages;

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
  Future<void> _ensureShardLoaded() async {
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

    await _ensureShardLoaded();

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

      // Send Round 1 + msg_hash via WebSocket
      final round1WithHash = [...round1.payload, ...msgHash];
      ws.sendRaw(toParty: _serverParty, round: 1, payload: round1WithHash);

      await MpcSessionStore.updateCurrentRound(1);

      // Wait for server's Round 1 (R_1)
      final serverR1 = await _waitForMessages(ws, expectedCount: 1);
      final serverR1Payload = serverR1.first.payload;

      // Process R_1 and generate Round 2
      final round2Payload = await MpcBridge.signProcessRound1AndGenerateRound2(
        localSessionId,
        serverR1Payload,
      );

      // Send DeviceContribution
      ws.sendRaw(toParty: _serverParty, round: 2, payload: round2Payload);

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
    // Ensure the current device shard is loaded into Rust memory before reshare
    await _ensureShardLoaded();

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

    final ws = MpcWebSocket(sessionId: remoteSessionId, partyIndex: _deviceParty, disableAutoReconnect: true);
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

      // Persist the new device shard to hardware-backed storage
      final newShardBytes = await MpcBridge.exportDeviceShard();
      await SecureHardware.storeDeviceShard(Uint8List.fromList(newShardBytes));

      // Update stored address (should be unchanged)
      await SecureStorage.save('mpc_address', walletInfo.address);

      // Update public key in case it changed (shouldn't, but be safe)
      final pubKeyHex = walletInfo.publicKey
          .map((b) => b.toRadixString(16).padLeft(2, '0'))
          .join();
      await SecureStorage.save('mpc_public_key', pubKeyHex);

      // Clear session state on success
      await MpcSessionStore.clearSession();

      // Record key usage for health tracking
      final health = KeyHealthService();
      health.recordPhoneKeyUsage();
      health.recordServerKeyUsage();

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
  /// [onProgress] 可选的进度回调 (completedCount, totalCount)
  Future<int> runPresign({
    required String walletId,
    int count = 5,
    void Function(int completed, int total)? onProgress,
  }) async {
    // Ensure device shard is loaded into Rust memory before protocol execution
    await _ensureShardLoaded();

    int generated = 0;

    for (int i = 0; i < count; i++) {
      final sessionResult = await MpcApi.createSession(
        sessionType: 'presign',
        parties: [_deviceParty, _serverParty],
        threshold: 2,
        walletId: walletId,
      );

      if (!sessionResult.isSuccess || sessionResult.data == null) {
        if (generated == 0) {
          throw MpcException(
            'Failed to create presign session: ${sessionResult.errorMessage}',
          );
        }
        // Partial success: some presigns generated before failure
        break;
      }

      final remoteSessionId = sessionResult.data!['session_id'] as String;
      _currentSessionId = remoteSessionId;

      final ws = MpcWebSocket(sessionId: remoteSessionId, partyIndex: _deviceParty);
      try {
        await ws.connect();

        // Generate presign Round 1
        final round1 = await MpcBridge.presignGenerateRound1();
        final localSessionId = round1.sessionId;

        // Save session state for crash recovery
        await MpcSessionStore.saveSession(MpcSessionState(
          sessionId: localSessionId,
          remoteSessionId: remoteSessionId,
          sessionType: 'presign',
          currentRound: 0,
          createdAt: DateTime.now(),
          metadata: {'wallet_id': walletId, 'index': i, 'total': count},
        ));

        // Send Round 1 to server
        ws.sendRaw(toParty: _serverParty, round: 1, payload: round1.payload);
        await MpcSessionStore.updateCurrentRound(1);

        // Wait for server's Round 1
        final serverR1 = await _waitForMessages(ws, expectedCount: 1);

        // Process and generate Round 2
        final round2Payload = await MpcBridge.presignProcessRound1AndGenerateRound2(
          localSessionId,
          serverR1.first.payload,
        );

        // Send Round 2
        ws.sendRaw(toParty: _serverParty, round: 2, payload: round2Payload);
        await MpcSessionStore.updateCurrentRound(2);

        // Finalize presignature and extract presig data
        final presigData = await MpcBridge.presignFinalize(localSessionId);

        // Upload presign data to server for storage
        await MpcApi.storePresignData(
          walletId: walletId,
          sessionId: remoteSessionId,
          presigData: presigData,
        );

        generated++;
        onProgress?.call(generated, count);

        // Clear session state on success for this iteration
        await MpcSessionStore.clearSession();
      } catch (e) {
        print('[MpcWalletService] Presign error (${i + 1}/$count): $e');
        if (generated == 0) {
          // No presigns generated at all - throw for UI to handle
          throw MpcSessionInterruptedException(
            'Presign session interrupted: $e',
            sessionState: await MpcSessionStore.loadSession(),
          );
        }
        // Partial success - stop the loop and return what we got
        break;
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
        );
      }
      rethrow;
    }
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
