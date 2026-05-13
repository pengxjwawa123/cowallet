# BIP-39 Mnemonic Implementation

This document describes the BIP-39 mnemonic encoding/decoding implementation added to the cowallet mobile app.

## Overview

BIP-39 (Bitcoin Improvement Proposal 39) provides a standard way to represent cryptographic entropy as human-readable mnemonic phrases. This implementation enables users to backup and recover their MPC wallet shards using 12 or 24 word phrases instead of raw hex strings.

## Changes Made

### 1. Added BIP-39 Dependency

**File**: `mobile/pubspec.yaml`

```yaml
dependencies:
  bip39: ^1.0.6  # Added under crypto utilities
```

### 2. Created BIP-39 Utility Module

**File**: `mobile/lib/utils/bip39.dart`

A comprehensive BIP-39 utility class with the following methods:

- `validateMnemonic(String mnemonic)` → `bool`
  - Validates a BIP-39 mnemonic phrase (checks wordlist and checksum)
  
- `mnemonicToEntropy(String mnemonic)` → `List<int>`
  - Converts mnemonic words to raw entropy bytes
  - 12 words → 16 bytes (128-bit entropy)
  - 24 words → 32 bytes (256-bit entropy)
  
- `entropyToMnemonic(List<int> entropy)` → `String`
  - Converts entropy bytes to a mnemonic phrase
  - 16 bytes → 12 words
  - 32 bytes → 24 words
  
- `mnemonicToSeed(String mnemonic, {String passphrase})` → `Uint8List`
  - Derives a 64-byte BIP-32 seed using PBKDF2-HMAC-SHA512
  - Optional passphrase support ("25th word")
  
- `generateMnemonic({int strength})` → `String`
  - Generates a new random mnemonic
  - Default: 128 bits (12 words)
  
- Helper methods for word count ↔ entropy size conversions

### 3. Updated Onboarding Flow

**File**: `mobile/lib/onboarding/onboarding_flow.dart`

**Changes**:
- Added import for `../utils/bip39.dart`
- Replaced placeholder `_mnemonicToBytes()` method with proper BIP-39 decoding
- Added validation: shows error toast if mnemonic is invalid
- Converts mnemonic to entropy using `Bip39.mnemonicToEntropy()`
- Validates entropy size matches expected word count (12 words → 16 bytes, 24 words → 32 bytes)

**Before**:
```dart
List<int> _mnemonicToBytes(List<String> words) {
  // Simplified implementation - placeholder
  final bytes = <int>[];
  for (var i = 0; i < 32; i++) {
    final wordIndex = i % words.length;
    final word = words[wordIndex];
    bytes.add((word.codeUnitAt(0) + i) % 256);
  }
  return bytes;
}
```

**After**:
```dart
// In _submitImport():
final mnemonic = _importCtrl.text.trim();

// Validate BIP-39 mnemonic first
if (!Bip39.validateMnemonic(mnemonic)) {
  showTopToast(context, 'Invalid recovery phrase...', ...);
  return;
}

// Convert to entropy bytes
final backupBytes = Bip39.mnemonicToEntropy(mnemonic);

// Verify expected size
final expectedSize = _wordCount == 24 ? 32 : 16;
if (backupBytes.length != expectedSize) {
  throw Exception('Invalid entropy size...');
}
```

### 4. Enhanced Backup Shard Service

**File**: `mobile/lib/services/backup_shard_service.dart`

**Changes**:
- Added import for `../utils/bip39.dart`
- Modified `storeBackupShard()` to generate BIP-39 mnemonic from shard bytes
- Stores mnemonic in secure storage automatically
- Includes mnemonic in backup payload JSON (both hex and mnemonic)
- Updated `BackupResult` class to include mnemonic field

**Key Addition**:
```dart
// Generate BIP-39 mnemonic from the shard bytes for user-friendly backup
String? mnemonic;
try {
  mnemonic = Bip39.entropyToMnemonic(shardBytes);
  await SecureStorage.saveMnemonic(mnemonic);
} catch (e) {
  print('Warning: Failed to generate mnemonic: $e');
}

final payload = _buildBackupPayload(shardHex, mnemonic: mnemonic);
```

**Updated JSON Structure**:
```json
{
  "version": 1,
  "type": "cowallet_backup_shard",
  "shard": "hex_encoded_bytes",
  "mnemonic": "word1 word2 ... word12",  // NEW
  "created_at": "2026-05-11T12:00:00Z"
}
```

### 5. Added Comprehensive Tests

**File**: `mobile/test/utils/bip39_test.dart`

Test coverage includes:
- Mnemonic validation (valid/invalid cases)
- Entropy ↔ Mnemonic conversions
- Round-trip testing
- Seed derivation with/without passphrase
- Random mnemonic generation
- Helper function testing
- Error handling

## Usage Examples

### Backup Creation (Encoding)

```dart
// During wallet creation, convert shard bytes to mnemonic
final shardBytes = await MpcBridge.exportBackupShard();  // 32 bytes
final mnemonic = Bip39.entropyToMnemonic(shardBytes);
// Result: "word1 word2 word3 ... word24"
```

### Recovery Import (Decoding)

```dart
// User enters recovery phrase
final mnemonic = "word1 word2 word3 ... word24";

// Validate first
if (!Bip39.validateMnemonic(mnemonic)) {
  throw Exception('Invalid recovery phrase');
}

// Convert to bytes for MPC recovery
final shardBytes = Bip39.mnemonicToEntropy(mnemonic);  // 32 bytes
await MpcBridge.recoveryImportBackupShard(shardBytes);
```

## Security Considerations

1. **Entropy Quality**: The BIP-39 implementation uses cryptographically secure random number generation from the `bip39` package.

2. **Checksum Validation**: The last word(s) of a mnemonic encode a checksum, preventing typos and corruption.

3. **Secure Storage**: Mnemonics are stored in platform-specific secure storage:
   - iOS: Keychain with `first_unlock` accessibility
   - Android: EncryptedSharedPreferences

4. **Passphrase Support**: Optional BIP-39 passphrase ("25th word") supported for seed derivation, though not currently used for MPC shard backup (which uses raw entropy).

## Word Count Mapping

| Entropy Size | Checksum Bits | Total Bits | Word Count |
|--------------|---------------|------------|------------|
| 128 bits     | 4 bits        | 132 bits   | 12 words   |
| 160 bits     | 5 bits        | 165 bits   | 15 words   |
| 192 bits     | 6 bits        | 198 bits   | 18 words   |
| 224 bits     | 7 bits        | 231 bits   | 21 words   |
| 256 bits     | 8 bits        | 264 bits   | 24 words   |

## Next Steps

To complete the implementation, run:

```bash
cd mobile
flutter pub get          # Install bip39 package
flutter test             # Run tests including BIP-39 tests
flutter analyze          # Verify no issues
```

## References

- [BIP-39 Specification](https://github.com/bitcoin/bips/blob/master/bip-0039.mediawiki)
- [BIP-39 English Wordlist](https://github.com/bitcoin/bips/blob/master/bip-0039/english.txt)
- [bip39 Dart Package](https://pub.dev/packages/bip39)
