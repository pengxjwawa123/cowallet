import 'package:flutter_test/flutter_test.dart';
import 'package:cowallet/utils/bip39.dart';

void main() {
  group('Bip39', () {
    test('validates correct 12-word mnemonic', () {
      const mnemonic = 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about';
      expect(Bip39.validateMnemonic(mnemonic), isTrue);
    });

    test('validates correct 24-word mnemonic', () {
      const mnemonic = 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art';
      expect(Bip39.validateMnemonic(mnemonic), isTrue);
    });

    test('rejects invalid mnemonic with bad checksum', () {
      const mnemonic = 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon';
      expect(Bip39.validateMnemonic(mnemonic), isFalse);
    });

    test('rejects mnemonic with invalid word', () {
      const mnemonic = 'invalid word here that does not exist in wordlist foo bar baz qux';
      expect(Bip39.validateMnemonic(mnemonic), isFalse);
    });

    test('converts 12-word mnemonic to 16-byte entropy', () {
      const mnemonic = 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about';
      final entropy = Bip39.mnemonicToEntropy(mnemonic);
      expect(entropy.length, equals(16));
      expect(entropy, equals([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]));
    });

    test('converts 24-word mnemonic to 32-byte entropy', () {
      const mnemonic = 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art';
      final entropy = Bip39.mnemonicToEntropy(mnemonic);
      expect(entropy.length, equals(32));
    });

    test('converts 16-byte entropy to 12-word mnemonic', () {
      final entropy = List<int>.filled(16, 0);
      final mnemonic = Bip39.entropyToMnemonic(entropy);
      expect(mnemonic.split(' ').length, equals(12));
      expect(mnemonic, equals('abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about'));
    });

    test('converts 32-byte entropy to 24-word mnemonic', () {
      final entropy = List<int>.filled(32, 0);
      final mnemonic = Bip39.entropyToMnemonic(entropy);
      expect(mnemonic.split(' ').length, equals(24));
      expect(mnemonic, equals('abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art'));
    });

    test('round-trip: entropy → mnemonic → entropy', () {
      final originalEntropy = List<int>.generate(32, (i) => i % 256);
      final mnemonic = Bip39.entropyToMnemonic(originalEntropy);
      final recoveredEntropy = Bip39.mnemonicToEntropy(mnemonic);
      expect(recoveredEntropy, equals(originalEntropy));
    });

    test('throws on invalid entropy length', () {
      expect(
        () => Bip39.entropyToMnemonic([1, 2, 3]),
        throwsArgumentError,
      );
    });

    test('throws on invalid mnemonic for mnemonicToEntropy', () {
      expect(
        () => Bip39.mnemonicToEntropy('invalid mnemonic here'),
        throwsArgumentError,
      );
    });

    test('generates valid random mnemonic with default strength', () {
      final mnemonic = Bip39.generateMnemonic();
      expect(Bip39.validateMnemonic(mnemonic), isTrue);
      expect(mnemonic.split(' ').length, equals(12));
    });

    test('generates valid 24-word mnemonic with 256-bit strength', () {
      final mnemonic = Bip39.generateMnemonic(strength: 256);
      expect(Bip39.validateMnemonic(mnemonic), isTrue);
      expect(mnemonic.split(' ').length, equals(24));
    });

    test('derives 64-byte seed from mnemonic', () {
      const mnemonic = 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about';
      final seed = Bip39.mnemonicToSeed(mnemonic);
      expect(seed.length, equals(64));
      // Known test vector for this mnemonic with empty passphrase
      expect(seed.sublist(0, 8), equals([92, 225, 50, 96, 151, 250, 49, 139]));
    });

    test('derives different seed with passphrase', () {
      const mnemonic = 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about';
      final seedWithoutPassphrase = Bip39.mnemonicToSeed(mnemonic);
      final seedWithPassphrase = Bip39.mnemonicToSeed(mnemonic, passphrase: 'test');
      expect(seedWithoutPassphrase, isNot(equals(seedWithPassphrase)));
    });

    test('getWordCountForEntropySize returns correct values', () {
      expect(Bip39.getWordCountForEntropySize(16), equals(12));
      expect(Bip39.getWordCountForEntropySize(20), equals(15));
      expect(Bip39.getWordCountForEntropySize(24), equals(18));
      expect(Bip39.getWordCountForEntropySize(28), equals(21));
      expect(Bip39.getWordCountForEntropySize(32), equals(24));
      expect(Bip39.getWordCountForEntropySize(10), isNull);
    });

    test('getEntropySizeForWordCount returns correct values', () {
      expect(Bip39.getEntropySizeForWordCount(12), equals(16));
      expect(Bip39.getEntropySizeForWordCount(15), equals(20));
      expect(Bip39.getEntropySizeForWordCount(18), equals(24));
      expect(Bip39.getEntropySizeForWordCount(21), equals(28));
      expect(Bip39.getEntropySizeForWordCount(24), equals(32));
      expect(Bip39.getEntropySizeForWordCount(10), isNull);
    });
  });
}
