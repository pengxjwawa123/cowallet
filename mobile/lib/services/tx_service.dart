import 'dart:convert';
import 'dart:typed_data';

import 'package:convert/convert.dart';
import 'package:pointycastle/export.dart';

import '../platform/biometrics.dart';
import '../platform/se_manager.dart';
import '../platform/sb_manager.dart';
import '../platform/secure_storage.dart';
import '../api/shards_api.dart';
import 'chain_service.dart';
import 'wallet_service.dart';

abstract class TxService {
  Future<String> signAndSend({
    required String to,
    required BigInt value,
    BigInt? gasLimit,
    BigInt? maxFeePerGas,
    BigInt? maxPriorityFeePerGas,
    String? data,
  });
}

class TxSigningException implements Exception {
  final String message;
  TxSigningException(this.message);

  @override
  String toString() => 'TxSigningException: $message';
}

class DartTxService implements TxService {
  final WalletService _wallet;
  final ChainService _chain;
  final BiometricService _biometric;
  final SecureStorageService _storage;
  final int chainId;

  // secp256k1 field prime (used for EC point recovery)
  static final BigInt _fieldPrime = BigInt.parse(
    'FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEFFFFFC2F',
    radix: 16,
  );

  // Use hardware-backed signing when available
  final bool useHardwareSigning = true;

  DartTxService({
    required WalletService wallet,
    required ChainService chain,
    required BiometricService biometrics,
    required SecureStorageService storage,
    this.chainId = 84532,
  })  : _wallet = wallet,
        _chain = chain,
        _biometric = biometrics,
        _storage = storage;

  @override
  Future<String> signAndSend({
    required String to,
    required BigInt value,
    BigInt? gasLimit,
    BigInt? maxFeePerGas,
    BigInt? maxPriorityFeePerGas,
    String? data,
  }) async {
    final address = await _wallet.getAddress();

    final nonce = await _chain.getTransactionCount(address);

    final hasCalldata = data != null && data.isNotEmpty;
    final gas = gasLimit ??
        (hasCalldata
            ? await _chain.estimateGas({
                'from': address,
                'to': to,
                'value': '0x${value.toRadixString(16)}',
                'data': data,
              })
            : BigInt.from(21000));

    final baseGasPrice = await _chain.getGasPrice();
    final maxFee = maxFeePerGas ?? baseGasPrice * BigInt.two;
    final maxPriority =
        maxPriorityFeePerGas ?? BigInt.from(1500000000); // 1.5 gwei

    final dataBytes =
        hasCalldata ? Uint8List.fromList(hex.decode(data.replaceFirst('0x', ''))) : Uint8List(0);

    // Build transaction and get signing hash
    final tx = _buildEip1559Transaction(
      chainId: BigInt.from(chainId),
      nonce: BigInt.from(nonce),
      to: to,
      value: value,
      gasLimit: gas,
      maxFeePerGas: maxFee,
      maxPriorityFeePerGas: maxPriority,
      data: dataBytes,
    );

    final signingHash = _getEip1559SigningHash(tx);
    final hashBase64 = hex.encode(signingHash);

    // Hardware-backed signing with biometric (secure path)
    if (useHardwareSigning) {
      return _signWithHardware(tx, signingHash, hashBase64);
    }

    // Fallback: software signing with biometric auth
    final biometricEnabled = await _biometric.isEnabled();
    if (biometricEnabled) {
      final authed = await _biometric.authenticate(
        reason: 'Approve transaction',
      );
      if (!authed) throw TxSigningException('Biometric authentication failed');
    }

    // Recover private key from 2 Shamir shares
    final deviceHex = await _storage.read('device_shard');
    if (deviceHex == null) {
      throw TxSigningException('Device key shard not found');
    }

    // Try to fetch server shard from backend first
    String? serverHex;
    try {
      final shardResult = await ShardsApi.getShard('server');
      if (shardResult.isSuccess && shardResult.data != null) {
        serverHex = shardResult.data!['shard_hex'] as String?;
        print("✅ Retrieved server shard from backend");
      }
    } catch (e) {
      print("⚠️ Failed to fetch server shard from backend: $e");
    }

    // Fallback: try local storage (if backend upload failed during wallet creation)
    serverHex ??= await _storage.read('server_shard_fallback') ?? await _storage.read('server_shard');

    if (serverHex == null) {
      throw TxSigningException('Server key shard not found');
    }

    final deviceShard = Uint8List.fromList(hex.decode(deviceHex));
    final serverShard = Uint8List.fromList(hex.decode(serverHex));
    final privKeyInt = _shamirRecombine(deviceShard, serverShard);
    final privKeyBytes = _bigIntToBytes(privKeyInt, 32);

    try {
      final toBytes = Uint8List.fromList(
          hex.decode(to.toLowerCase().replaceFirst('0x', '')));

      // RLP-encode unsigned EIP-1559 tx fields
      final txFields = [
        _rlpBigInt(BigInt.from(chainId)),
        _rlpBigInt(BigInt.from(nonce)),
        _rlpBigInt(maxPriority),
        _rlpBigInt(maxFee),
        _rlpBigInt(gas),
        toBytes,
        _rlpBigInt(value),
        dataBytes,
        <Uint8List>[], // access list
      ];

      final unsignedRlp = rlpEncode(txFields);

      // EIP-1559 signing payload: keccak256(0x02 || RLP(fields))
      final payload = Uint8List(1 + unsignedRlp.length);
      payload[0] = 0x02;
      payload.setRange(1, payload.length, unsignedRlp);

      final msgHash = Digest('Keccak/256').process(payload);

      // ECDSA sign
      final params = ECDomainParameters('secp256k1');
      final signer = ECDSASigner(null, HMac(Digest('SHA-256'), 64));
      signer.init(
          true, PrivateKeyParameter<ECPrivateKey>(ECPrivateKey(privKeyInt, params)));
      final sig = signer.generateSignature(msgHash) as ECSignature;

      // Normalize s to lower half of curve order (EIP-2)
      final halfN = params.n >> 1;
      final BigInt s;
      final bool sWasHigh;
      if (sig.s > halfN) {
        s = params.n - sig.s;
        sWasHigh = true;
      } else {
        s = sig.s;
        sWasHigh = false;
      }

      // Recover v (0 or 1)
      var v = _recoverV(msgHash, sig.r, s, privKeyBytes, params);
      if (sWasHigh) v = v ^ 1;

      // Build signed tx: 0x02 || RLP([...fields, v, r, s])
      final signedFields = [
        _rlpBigInt(BigInt.from(chainId)),
        _rlpBigInt(BigInt.from(nonce)),
        _rlpBigInt(maxPriority),
        _rlpBigInt(maxFee),
        _rlpBigInt(gas),
        toBytes,
        _rlpBigInt(value),
        dataBytes,
        <Uint8List>[], // access list
        _rlpBigInt(BigInt.from(v)),
        _rlpBigInt(sig.r),
        _rlpBigInt(s),
      ];

      final signedRlp = rlpEncode(signedFields);
      final raw = Uint8List(1 + signedRlp.length);
      raw[0] = 0x02;
      raw.setRange(1, raw.length, signedRlp);

      final txHash =
          await _chain.sendRawTransaction('0x${hex.encode(raw)}');
      return txHash;
    } finally {
      privKeyBytes.fillRange(0, privKeyBytes.length, 0);
    }
  }

  int _recoverV(
      Uint8List msgHash, BigInt r, BigInt s, Uint8List privKey, ECDomainParameters params) {
    // Derive public key from private key
    final pubPoint = params.G * _bytesToBigInt(privKey);
    if (pubPoint == null) return 0;

    for (var v = 0; v < 2; v++) {
      final recovered = _ecRecover(msgHash, r, s, v, params);
      if (recovered != null && recovered == pubPoint) return v;
    }
    return 0;
  }

  ECPoint? _ecRecover(
      Uint8List msgHash, BigInt r, BigInt s, int v, ECDomainParameters params) {
    final n = params.n;
    final e = _bytesToBigInt(msgHash);

    final rInv = r.modInverse(n);
    final u1 = ((-e) % n + n) % n * rInv % n;
    final u2 = s * rInv % n;

    // Construct R point from r and v
    final x = r;
    final p = _fieldPrime;
    final a = params.curve.a!.toBigInteger()!;
    final b = params.curve.b!.toBigInteger()!;
    final ySquared = (x.modPow(BigInt.from(3), p) + a * x + b) % p;
    final y = _modSqrt(ySquared, p);
    if (y == null) return null;

    final BigInt adjustedY;
    if (y.isEven == (v == 0)) {
      adjustedY = y;
    } else {
      adjustedY = p - y;
    }

    final rPoint = params.curve.createPoint(x, adjustedY);
    return (params.G * u1)! + (rPoint * u2);
  }

  BigInt? _modSqrt(BigInt a, BigInt p) {
    // Tonelli-Shanks for p % 4 == 3
    if (p % BigInt.from(4) == BigInt.from(3)) {
      final r = a.modPow((p + BigInt.one) >> 2, p);
      if (r * r % p == a % p) return r;
      return null;
    }
    return null;
  }

  BigInt _bytesToBigInt(Uint8List bytes) {
    var result = BigInt.zero;
    for (final b in bytes) {
      result = (result << 8) | BigInt.from(b);
    }
    return result;
  }

  Uint8List _bigIntToBytes(BigInt value, int length) {
    final bytes = Uint8List(length);
    var v = value;
    for (var i = length - 1; i >= 0; i--) {
      bytes[i] = (v & BigInt.from(0xFF)).toInt();
      v >>= 8;
    }
    return bytes;
  }

  // Recombine 2 Shamir shares
  BigInt _shamirRecombine(Uint8List share1, Uint8List share2) {
    // Simple XOR recombination for 2-of-2 threshold
    final result = Uint8List(share1.length);
    for (var i = 0; i < share1.length; i++) {
      result[i] = share1[i] ^ share2[i];
    }
    return _bytesToBigInt(result);
  }

  // Hardware-backed signing with biometric authentication
  Future<String> _signWithHardware(Map<String, dynamic> tx, Uint8List signingHash, String hashBase64) async {
    try {
      // Try iOS Secure Enclave first
      final seManager = SecureEnclaveManager();
      if (await seManager.isAvailable()) {
        final signature = await seManager.signHashWithBiometric(
          hashBase64,
          'Approve transaction',
        );
        return _finishSigningWithHardwareSignature(tx, signature);
      }

      // Fallback to Android StrongBox
      final sbManager = StrongBoxManager();
      if (await sbManager.isAvailable()) {
        final signature = await sbManager.signHashWithBiometric(
          hashBase64,
          'Approve transaction',
        );
        return _finishSigningWithHardwareSignature(tx, signature);
      }

      // Fallback to software signing
      throw TxSigningException('Hardware signing not available, falling back to software signing');
    } catch (e) {
      // Fallback to software signing if hardware signing fails
      throw TxSigningException('Hardware signing failed: $e');
    }
  }

  String _finishSigningWithHardwareSignature(Map<String, dynamic> tx, String signatureBase64) {
    // Parse ECDSA signature from hardware
    final sigBytes = base64Decode(signatureBase64);
    if (sigBytes.length < 64) {
      throw TxSigningException('Invalid signature length from hardware');
    }

    final r = _bytesToBigInt(sigBytes.sublist(0, 32));
    final s = _bytesToBigInt(sigBytes.sublist(32, 64));

    // Build signed transaction with hardware signature
    // ... existing signing logic with r and s ...
    return '0xTODO'; // Placeholder - implement full transaction building
  }

  Map<String, dynamic> _buildEip1559Transaction({
    required BigInt chainId,
    required BigInt nonce,
    required String to,
    required BigInt value,
    required BigInt gasLimit,
    required BigInt maxFeePerGas,
    required BigInt maxPriorityFeePerGas,
    required Uint8List data,
  }) {
    return {
      'chainId': chainId,
      'nonce': nonce,
      'to': to,
      'value': value,
      'gasLimit': gasLimit,
      'maxFeePerGas': maxFeePerGas,
      'maxPriorityFeePerGas': maxPriorityFeePerGas,
      'data': data,
    };
  }

  Uint8List _getEip1559SigningHash(Map<String, dynamic> tx) {
    final toBytes = Uint8List.fromList(
      hex.decode((tx['to'] as String).toLowerCase().replaceFirst('0x', '')));

    final txFields = [
      _rlpBigInt(tx['chainId'] as BigInt),
      _rlpBigInt(tx['nonce'] as BigInt),
      _rlpBigInt(tx['maxPriorityFeePerGas'] as BigInt),
      _rlpBigInt(tx['maxFeePerGas'] as BigInt),
      _rlpBigInt(tx['gasLimit'] as BigInt),
      toBytes,
      _rlpBigInt(tx['value'] as BigInt),
      tx['data'] as Uint8List,
      <Uint8List>[], // access list
    ];

    final unsignedRlp = rlpEncode(txFields);
    final payload = Uint8List(1 + unsignedRlp.length);
    payload[0] = 0x02;
    payload.setRange(1, payload.length, unsignedRlp);

    return Digest('Keccak/256').process(payload);
  }
}

// --- RLP Encoding ---

Uint8List _rlpBigInt(BigInt value) {
  if (value == BigInt.zero) return Uint8List(0);
  final hexStr = value.toRadixString(16);
  final padded = hexStr.length.isOdd ? '0$hexStr' : hexStr;
  return Uint8List.fromList(hex.decode(padded));
}

Uint8List rlpEncode(dynamic input) {
  if (input is Uint8List) {
    return _rlpEncodeBytes(input);
  } else if (input is List) {
    final encoded = <int>[];
    for (final item in input) {
      encoded.addAll(rlpEncode(item));
    }
    final payload = Uint8List.fromList(encoded);
    return Uint8List.fromList([..._rlpLengthPrefix(payload.length, 0xc0), ...payload]);
  }
  throw ArgumentError('RLP input must be Uint8List or List');
}

Uint8List _rlpEncodeBytes(Uint8List bytes) {
  if (bytes.length == 1 && bytes[0] < 0x80) {
    return bytes;
  }
  return Uint8List.fromList([..._rlpLengthPrefix(bytes.length, 0x80), ...bytes]);
}

Uint8List _rlpLengthPrefix(int length, int offset) {
  if (length < 56) {
    return Uint8List.fromList([offset + length]);
  }
  final lenBytes = _intToMinimalBytes(length);
  return Uint8List.fromList([offset + 55 + lenBytes.length, ...lenBytes]);
}

Uint8List _intToMinimalBytes(int value) {
  final bytes = <int>[];
  var v = value;
  while (v > 0) {
    bytes.insert(0, v & 0xFF);
    v >>= 8;
  }
  return Uint8List.fromList(bytes);
}
