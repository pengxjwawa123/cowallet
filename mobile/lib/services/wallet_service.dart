import 'dart:math';
import 'dart:typed_data';

import 'package:convert/convert.dart';
import 'package:pointycastle/export.dart';

import '../platform/secure_storage.dart';

class WalletKeys {
  final String address;
  final Uint8List publicKey;
  final List<Uint8List> shards;

  WalletKeys({
    required this.address,
    required this.publicKey,
    required this.shards,
  });
}

abstract class WalletService {
  Future<WalletKeys> generateWallet();
  Future<String> getAddress();
  Future<bool> hasWallet();
  Future<void> deleteWallet();
}

class DartWalletService implements WalletService {
  final SecureStorageService _storage;

  static final ECDomainParameters _params = ECDomainParameters('secp256k1');

  DartWalletService(this._storage);

  @override
  Future<WalletKeys> generateWallet() async {
    final keyGen = ECKeyGenerator()
      ..init(ParametersWithRandom(
        ECKeyGeneratorParameters(_params),
        _secureRandom(),
      ));

    final pair = keyGen.generateKeyPair();
    final privateKey = pair.privateKey as ECPrivateKey;
    final publicKey = pair.publicKey as ECPublicKey;

    final pubBytes = publicKey.Q!.getEncoded(false);
    final address = _deriveAddress(pubBytes);

    final privBytes = _bigIntToBytes(privateKey.d!, 32);
    final shards = _shamirSplit(privBytes, 3, 2);

    // Zero the raw private key immediately
    privBytes.fillRange(0, privBytes.length, 0);

    await _storage.write('device_shard', hex.encode(shards[0]));
    await _storage.write('server_shard', hex.encode(shards[1]));
    await _storage.write('wallet_address', address);
    await _storage.write('wallet_pubkey', hex.encode(pubBytes));

    return WalletKeys(
      address: address,
      publicKey: pubBytes,
      shards: shards,
    );
  }

  @override
  Future<String> getAddress() async {
    final addr = await _storage.read('wallet_address');
    if (addr == null) throw StateError('No wallet found');
    return addr;
  }

  @override
  Future<bool> hasWallet() async {
    return _storage.containsKey('wallet_address');
  }

  @override
  Future<void> deleteWallet() async {
    await _storage.delete('device_shard');
    await _storage.delete('server_shard');
    await _storage.delete('wallet_address');
    await _storage.delete('wallet_pubkey');
  }

  // --- Ethereum address derivation ---

  static String _deriveAddress(Uint8List uncompressedPubKey) {
    // Drop the 0x04 prefix
    final keyBody = uncompressedPubKey.sublist(1);
    final digest = Digest('Keccak/256');
    final hashed = digest.process(keyBody);
    final addrBytes = hashed.sublist(12);
    final addrHex = hex.encode(addrBytes);
    return _eip55Checksum(addrHex);
  }

  static String _eip55Checksum(String addrHex) {
    final digest = Digest('Keccak/256');
    final hashBytes = digest.process(Uint8List.fromList(addrHex.codeUnits));
    final hashHex = hex.encode(hashBytes);

    final buf = StringBuffer('0x');
    for (var i = 0; i < addrHex.length; i++) {
      final c = addrHex[i];
      if (int.parse(hashHex[i], radix: 16) >= 8) {
        buf.write(c.toUpperCase());
      } else {
        buf.write(c.toLowerCase());
      }
    }
    return buf.toString();
  }

  // --- Shamir Secret Sharing over GF(p), p = secp256k1 curve order ---

  static List<Uint8List> _shamirSplit(
      Uint8List secret, int totalShares, int threshold) {
    assert(threshold == 2, 'Only 2-of-n SSS implemented');
    final p = _params.n;
    final secretInt = _bytesToBigInt(secret);

    // Random coefficient a1 for polynomial f(x) = secret + a1*x mod p
    final rng = _secureRandom();
    BigInt a1;
    do {
      a1 = rng.nextBigInteger(256) % p;
    } while (a1 == BigInt.zero);

    final shares = <Uint8List>[];
    for (var x = 1; x <= totalShares; x++) {
      final xBig = BigInt.from(x);
      final y = (secretInt + a1 * xBig) % p;
      // Encode share as: 1 byte x || 32 bytes y
      final yBytes = _bigIntToBytes(y, 32);
      final share = Uint8List(33);
      share[0] = x;
      share.setRange(1, 33, yBytes);
      shares.add(share);
    }
    return shares;
  }

  // --- Helpers ---

  static SecureRandom _secureRandom() {
    final rng = FortunaRandom();
    final seed = Uint8List(32);
    final dartRng = Random.secure();
    for (var i = 0; i < 32; i++) {
      seed[i] = dartRng.nextInt(256);
    }
    rng.seed(KeyParameter(seed));
    return rng;
  }

  static Uint8List _bigIntToBytes(BigInt value, int length) {
    final result = Uint8List(length);
    var v = value;
    for (var i = length - 1; i >= 0; i--) {
      result[i] = (v & BigInt.from(0xFF)).toInt();
      v >>= 8;
    }
    return result;
  }

  static BigInt _bytesToBigInt(Uint8List bytes) {
    var result = BigInt.zero;
    for (final b in bytes) {
      result = (result << 8) | BigInt.from(b);
    }
    return result;
  }
}

/// Reconstruct secret from 2 Shamir shares via Lagrange interpolation.
BigInt shamirRecombine(Uint8List share1, Uint8List share2) {
  final p = ECDomainParameters('secp256k1').n;

  final x1 = BigInt.from(share1[0]);
  final y1 = _bytesToBigInt(share1.sublist(1));
  final x2 = BigInt.from(share2[0]);
  final y2 = _bytesToBigInt(share2.sublist(1));

  // Lagrange basis: L1 = -x2 / (x1 - x2), L2 = -x1 / (x2 - x1)
  final denom1 = (x1 - x2) % p;
  final denom2 = (x2 - x1) % p;

  final invDenom1 = denom1.modInverse(p);
  final invDenom2 = denom2.modInverse(p);

  final l1 = ((-x2) * invDenom1) % p;
  final l2 = ((-x1) * invDenom2) % p;

  final secret = ((y1 * l1) + (y2 * l2)) % p;
  return (secret + p) % p;
}

BigInt _bytesToBigInt(Uint8List bytes) {
  var result = BigInt.zero;
  for (final b in bytes) {
    result = (result << 8) | BigInt.from(b);
  }
  return result;
}

Uint8List bigIntToBytes(BigInt value, int length) {
  final result = Uint8List(length);
  var v = value;
  for (var i = length - 1; i >= 0; i--) {
    result[i] = (v & BigInt.from(0xFF)).toInt();
    v >>= 8;
  }
  return result;
}
