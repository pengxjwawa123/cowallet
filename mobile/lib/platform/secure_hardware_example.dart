// Example usage of SecureHardware interface
// This file demonstrates how to use hardware-backed secure storage for MPC shards

import 'dart:typed_data';
import 'secure_hardware.dart';

/// Example: Store and retrieve device shard with hardware encryption
Future<void> exampleStoreAndLoadShard() async {
  // Check if hardware security is available
  final available = await SecureHardware.isAvailable();
  if (!available) {
    print('Hardware security not available on this device');
    return;
  }

  // Get detailed security info
  final info = await SecureHardware.getInfo();
  print('Security level: ${info.securityLevel}');
  print('Biometric required: ${info.biometricRequired}');

  // Initialize hardware security (once during onboarding)
  await SecureHardware.initialize('device-123');

  // Example shard data (in production, this comes from MPC DKG)
  final shardBytes = Uint8List.fromList([
    0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
    0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10,
  ]);

  // Store encrypted shard (encrypted with hardware-backed key)
  await SecureHardware.storeDeviceShard(shardBytes);
  print('Shard stored securely');

  // Load decrypted shard (may require biometric auth)
  final loadedShard = await SecureHardware.loadDeviceShard();
  if (loadedShard != null) {
    print('Shard loaded: ${loadedShard.length} bytes');

    // Verify integrity
    final match = _compareBytes(shardBytes, loadedShard);
    print('Integrity check: ${match ? "PASS" : "FAIL"}');
  }

  // Sign a transaction hash with hardware-backed key
  final txHash = Uint8List(32); // 32-byte transaction hash
  try {
    final signature = await SecureHardware.signHash(
      txHash,
      'Sign transaction for 0.5 ETH transfer',
    );
    print('Signature: ${signature.length} bytes');
  } catch (e) {
    print('Signing failed (user may have cancelled biometric): $e');
  }
}

/// Example: Clear all hardware-backed data (wallet reset)
Future<void> exampleClearWallet() async {
  await SecureHardware.clear();
  print('All hardware-backed data cleared');
}

bool _compareBytes(Uint8List a, Uint8List b) {
  if (a.length != b.length) return false;
  for (var i = 0; i < a.length; i++) {
    if (a[i] != b[i]) return false;
  }
  return true;
}
