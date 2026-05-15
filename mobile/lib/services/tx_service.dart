import 'dart:typed_data';

import 'package:convert/convert.dart';
import 'package:pointycastle/export.dart';

import '../api/tx_api.dart';
import '../l10n/strings.dart';
import 'chain_service.dart';
import 'locator.dart';
import 'wallet_service.dart';

abstract class TxService {
  Future<String> signAndSend({
    required String to,
    required BigInt value,
    BigInt? gasLimit,
    BigInt? maxFeePerGas,
    BigInt? maxPriorityFeePerGas,
    String? data,
    int? chainId,
  });

  Future<String> sendErc20({
    required String to,
    required String tokenContract,
    required BigInt amount,
    BigInt? gasLimit,
    BigInt? maxFeePerGas,
    BigInt? maxPriorityFeePerGas,
    int? chainId,
  });
}

class TxSigningException implements Exception {
  final String message;
  TxSigningException(this.message);

  @override
  String toString() => 'TxSigningException: $message';
}

class MpcTxService implements TxService {
  final WalletService _wallet;
  final ChainService _chain;

  MpcTxService({
    required WalletService wallet,
    required ChainService chain,
  })  : _wallet = wallet,
        _chain = chain;

  /// The currently selected chain ID from the chain service.
  int get chainId => _chain.currentConfig.chainId;

  @override
  Future<String> signAndSend({
    required String to,
    required BigInt value,
    BigInt? gasLimit,
    BigInt? maxFeePerGas,
    BigInt? maxPriorityFeePerGas,
    String? data,
    int? chainId,
  }) async {
    final effectiveChainId = chainId ?? this.chainId;

    // Switch chain RPC if targeting a different chain
    if (_chain is JsonRpcChainService && effectiveChainId != this.chainId) {
      (_chain as JsonRpcChainService).switchChain(ChainConfig.byId(effectiveChainId));
    }

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

    final baseFee = await _chain.getBaseFee() ?? await _chain.getGasPrice();
    final maxPriority = maxPriorityFeePerGas ?? await _chain.getMaxPriorityFeePerGas();
    final maxFee = maxFeePerGas ?? baseFee * BigInt.two + maxPriority;

    final dataBytes =
        hasCalldata ? Uint8List.fromList(hex.decode(data.replaceFirst('0x', ''))) : Uint8List(0);

    // MANDATORY user authentication before signing — never skip
    final authed = await Services.authenticate(reason: S.biometricAuthReason);
    if (!authed) {
      throw TxSigningException('User authentication required to sign transaction');
    }

    // Build EIP-1559 unsigned transaction for signing hash
    final toBytes = Uint8List.fromList(
        hex.decode(to.toLowerCase().replaceFirst('0x', '')));

    final txFields = [
      _rlpBigInt(BigInt.from(effectiveChainId)),
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
    final payload = Uint8List(1 + unsignedRlp.length);
    payload[0] = 0x02;
    payload.setRange(1, payload.length, unsignedRlp);

    final msgHash = Digest('Keccak/256').process(payload);

    // MPC distributed signature (device + server cooperate, no full key ever exists)
    final signResult = await _wallet.signWithSession(msgHash.toList());

    if (signResult.signature.length != 65) {
      throw TxSigningException('Invalid MPC signature length: ${signResult.signature.length}');
    }

    final r = _bytesToBigInt(Uint8List.fromList(signResult.signature.sublist(0, 32)));
    final s = _bytesToBigInt(Uint8List.fromList(signResult.signature.sublist(32, 64)));
    final rawV = signResult.signature[64];
    final v = rawV >= 27 ? rawV - 27 : rawV;

    // Build signed tx: 0x02 || RLP([...fields, v, r, s])
    final signedFields = [
      _rlpBigInt(BigInt.from(effectiveChainId)),
      _rlpBigInt(BigInt.from(nonce)),
      _rlpBigInt(maxPriority),
      _rlpBigInt(maxFee),
      _rlpBigInt(gas),
      toBytes,
      _rlpBigInt(value),
      dataBytes,
      <Uint8List>[], // access list
      _rlpBigInt(BigInt.from(v)),
      _rlpBigInt(r),
      _rlpBigInt(s),
    ];

    final signedRlp = rlpEncode(signedFields);
    final raw = Uint8List(1 + signedRlp.length);
    raw[0] = 0x02;
    raw.setRange(1, raw.length, signedRlp);

    final rawHex = '0x${hex.encode(raw)}';

    // Submit via backend (records tx in database + broadcasts to chain)
    final nativeSymbol = _nativeSymbolForChain(effectiveChainId);
    final submitResult = await TxApi.submit(
      rawTx: rawHex,
      chainId: effectiveChainId,
      toAddr: to,
      value: value.toString(),
      token: nativeSymbol,
      fromAddr: address,
      mpcSessionId: signResult.sessionId,
    );

    if (!submitResult.isSuccess || submitResult.data == null) {
      throw TxSigningException(
        submitResult.errorMessage ?? 'Transaction submission failed',
      );
    }

    // Restore default chain RPC
    if (_chain is JsonRpcChainService && effectiveChainId != this.chainId) {
      (_chain as JsonRpcChainService).switchChain(ChainConfig.byId(this.chainId));
    }

    // Trigger presign pool check after successful signing (non-blocking)
    Services.presignPool.checkAndRefill();

    return submitResult.data!['tx_hash'] as String;
  }

  @override
  Future<String> sendErc20({
    required String to,
    required String tokenContract,
    required BigInt amount,
    BigInt? gasLimit,
    BigInt? maxFeePerGas,
    BigInt? maxPriorityFeePerGas,
    int? chainId,
  }) async {
    // ERC-20 transfer(address,uint256) selector = 0xa9059cbb
    final toStripped = to.toLowerCase().replaceFirst('0x', '').padLeft(64, '0');
    final amountHex = amount.toRadixString(16).padLeft(64, '0');
    final calldata = '0xa9059cbb$toStripped$amountHex';

    return signAndSend(
      to: tokenContract,
      value: BigInt.zero,
      data: calldata,
      gasLimit: gasLimit,
      maxFeePerGas: maxFeePerGas,
      maxPriorityFeePerGas: maxPriorityFeePerGas,
      chainId: chainId,
    );
  }

  BigInt _bytesToBigInt(Uint8List bytes) {
    var result = BigInt.zero;
    for (final b in bytes) {
      result = (result << 8) | BigInt.from(b);
    }
    return result;
  }

  static String _nativeSymbolForChain(int chainId) {
    switch (chainId) {
      case 137:
      case 80002:
        return 'POL';
      case 56:
        return 'BNB';
      default:
        return 'ETH';
    }
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
