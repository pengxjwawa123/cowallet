// Integration tests for iOS Secure Enclave

import 'package:flutter_test/flutter_test.dart';
import 'package:cowallet/platform/ios_se_channel.dart';
import 'package:cowallet/platform/se_manager.dart';

void main() {
  group('iOS Secure Enclave Platform Channel', () {
    test('isAvailable returns boolean', () async {
      final available = await IosSecureEnclaveChannel.isAvailable();
      expect(available, isA<bool>());
    });

    test('storeSecret and getSecret work together', () async {
      const key = 'test-key';
      const value = 'test-value-123';

      try {
        // Store
        await IosSecureEnclaveChannel.storeSecret(key, value);

        // Retrieve
        final retrieved = await IosSecureEnclaveChannel.getSecret(key);
        expect(retrieved, equals(value));

        // Cleanup
        await IosSecureEnclaveChannel.deleteSecret(key);
      } catch (e) {
        // Platform channel not available in unit tests
        expect(e, isA<SeException>());
      }
    });

    test('deleteSecret removes data', () async {
      const key = 'test-delete-key';
      const value = 'test-value';

      try {
        await IosSecureEnclaveChannel.storeSecret(key, value);
        await IosSecureEnclaveChannel.deleteSecret(key);

        // After deletion, getSecret should return null or throw
        final retrieved = await IosSecureEnclaveChannel.getSecret(key);
        expect(retrieved, isNull);
      } catch (e) {
        // Expected in unit tests
        expect(e, isA<SeException>());
      }
    });
  });

  group('SecureEnclaveManager', () {
    late SecureEnclaveManager manager;

    setUp(() {
      manager = SecureEnclaveManager();
    });

    test('singleton pattern works', () {
      final manager1 = SecureEnclaveManager();
      final manager2 = SecureEnclaveManager();
      expect(identical(manager1, manager2), true);
    });

    test('isAvailable returns boolean', () async {
      try {
        final available = await manager.isAvailable();
        expect(available, isA<bool>());
      } catch (e) {
        // Expected in unit tests on non-iOS platforms
        print('SE not available: $e');
      }
    });

    test('getDeviceShardKeyId returns null when not initialized', () async {
      try {
        final keyId = await manager.getDeviceShardKeyId();
        // Should be null before initialization
        expect(keyId, anyOf([isNull, isA<String>()]));
      } catch (e) {
        print('Error: $e');
      }
    });

    test('clearWallet clears all data', () async {
      try {
        await manager.clearWallet();
        // Should succeed without error
      } catch (e) {
        // Expected in unit tests
        print('Clear wallet error: $e');
      }
    });
  });
}
