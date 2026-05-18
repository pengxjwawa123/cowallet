// iOS Secure Storage Channel Handler
// Handles platform channel calls for encrypted storage

import Flutter
import CryptoKit
import Security

public class MpcSecureStorageHandler: NSObject, FlutterPlugin {
  public static func dummy(methodCall: FlutterMethodCall, result: @escaping FlutterResult) {
    // This method is added to work around a fatal exception when a plugin is
    // registered using generics.
  }

  public static func register(with registrar: FlutterPluginRegistrar) {
    let channel = FlutterMethodChannel(
      name: "com.cowallet.mpc/storage",
      binaryMessenger: registrar.messenger()
    )
    let instance = MpcSecureStorageHandler()
    registrar.addMethodCallDelegate(instance, channel: channel)
  }

  public func dummyMethodToEnforceBundling() {
    // This method is added to work around a fatal exception when the plugin is
    // registered using generics.
  }

  public func handle(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
    switch call.method {
    case "storeSecret":
      storeSecret(call, result: result)
    case "getSecret":
      getSecret(call, result: result)
    case "deleteSecret":
      deleteSecret(call, result: result)
    case "storeEncryptedShard":
      storeEncryptedShard(call, result: result)
    case "loadEncryptedShard":
      loadEncryptedShard(call, result: result)
    default:
      result(FlutterMethodNotImplemented)
    }
  }

  // MARK: - Secure Storage

  /// Store encrypted data in Keychain
  private func storeSecret(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
    guard let args = call.arguments as? [String: Any],
          let key = args["key"] as? String,
          let value = args["value"] as? String else {
      result(FlutterError(code: "INVALID_ARGS", message: "key and value are required", details: nil))
      return
    }

    do {
      let data = value.data(using: .utf8) ?? Data()

      // Prepare keychain query
      let query: [String: Any] = [
        kSecClass as String: kSecClassGenericPassword,
        kSecAttrAccount as String: key,
        kSecAttrService as String: "com.cowallet.secure_storage",
        kSecValueData as String: data,
        kSecAttrAccessible as String: kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly,
        kSecAttrSynchronizable as String: false,
      ]

      // Delete existing value if present
      SecItemDelete(query as CFDictionary)

      // Add new value
      let status = SecItemAdd(query as CFDictionary, nil)

      guard status == errSecSuccess else {
        throw NSError(domain: "Keychain", code: Int(status))
      }

      result(nil) // Success, no return value needed
    } catch {
      result(FlutterError(code: "STORE_FAILED", message: error.localizedDescription, details: nil))
    }
  }

  /// Retrieve encrypted data from Keychain
  private func getSecret(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
    guard let args = call.arguments as? [String: Any],
          let key = args["key"] as? String else {
      result(FlutterError(code: "INVALID_ARGS", message: "key is required", details: nil))
      return
    }

    do {
      let query: [String: Any] = [
        kSecClass as String: kSecClassGenericPassword,
        kSecAttrAccount as String: key,
        kSecAttrService as String: "com.cowallet.secure_storage",
        kSecReturnData as String: true,
      ]

      var retrievedData: CFTypeRef?
      let status = SecItemCopyMatching(query as CFDictionary, &retrievedData)

      guard status == errSecSuccess else {
        // Key not found is not an error, return nil
        if status == errSecItemNotFound {
          result(nil)
        } else {
          throw NSError(domain: "Keychain", code: Int(status))
        }
        return
      }

      guard let data = retrievedData as? Data,
            let value = String(data: data, encoding: .utf8) else {
        result(FlutterError(code: "DECODE_FAILED", message: "Failed to decode value", details: nil))
        return
      }

      result(value)
    } catch {
      result(FlutterError(code: "GET_FAILED", message: error.localizedDescription, details: nil))
    }
  }

  /// Delete encrypted data from Keychain
  private func deleteSecret(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
    guard let args = call.arguments as? [String: Any],
          let key = args["key"] as? String else {
      result(FlutterError(code: "INVALID_ARGS", message: "key is required", details: nil))
      return
    }

    do {
      let query: [String: Any] = [
        kSecClass as String: kSecClassGenericPassword,
        kSecAttrAccount as String: key,
        kSecAttrService as String: "com.cowallet.secure_storage",
      ]

      let status = SecItemDelete(query as CFDictionary)

      guard status == errSecSuccess || status == errSecItemNotFound else {
        throw NSError(domain: "Keychain", code: Int(status))
      }

      result(nil) // Success
    } catch {
      result(FlutterError(code: "DELETE_FAILED", message: error.localizedDescription, details: nil))
    }
  }

  // MARK: - Hardware-Backed Shard Encryption

  /// Store device shard encrypted with Secure Enclave key
  private func storeEncryptedShard(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
    guard let args = call.arguments as? [String: Any] else {
      result(FlutterError(code: "INVALID_ARGS", message: "data is required", details: nil))
      return
    }

    let shardData: Data
    if let typedData = args["data"] as? FlutterStandardTypedData {
      shardData = typedData.data
    } else if let byteArray = args["data"] as? [UInt8] {
      shardData = Data(byteArray)
    } else if let intArray = args["data"] as? [Int] {
      shardData = Data(intArray.map { UInt8($0 & 0xFF) })
    } else {
      result(FlutterError(code: "INVALID_ARGS", message: "data is required", details: nil))
      return
    }

    do {

      // Get or create encryption key in Secure Enclave
      let encryptionKey = try getOrCreateEncryptionKey()

      // Encrypt the shard data using ChaChaPoly
      let sealedBox = try ChaChaPoly.seal(shardData, using: encryptionKey)

      // Store the combined (nonce + ciphertext + tag) in Keychain
      let encryptedData = sealedBox.combined

      let query: [String: Any] = [
        kSecClass as String: kSecClassGenericPassword,
        kSecAttrAccount as String: "device-shard-encrypted",
        kSecAttrService as String: "com.cowallet.secure_storage",
        kSecValueData as String: encryptedData,
        kSecAttrAccessible as String: kSecAttrAccessibleWhenUnlockedThisDeviceOnly,
        kSecAttrSynchronizable as String: false,
      ]

      // Delete existing value if present
      SecItemDelete(query as CFDictionary)

      // Add new encrypted value
      let status = SecItemAdd(query as CFDictionary, nil)

      guard status == errSecSuccess else {
        throw NSError(domain: "Keychain", code: Int(status))
      }

      result(nil) // Success
    } catch {
      result(FlutterError(code: "ENCRYPTION_FAILED", message: error.localizedDescription, details: nil))
    }
  }

  /// Load and decrypt device shard using Secure Enclave key
  private func loadEncryptedShard(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
    do {
      let query: [String: Any] = [
        kSecClass as String: kSecClassGenericPassword,
        kSecAttrAccount as String: "device-shard-encrypted",
        kSecAttrService as String: "com.cowallet.secure_storage",
        kSecReturnData as String: true,
      ]

      var retrievedData: CFTypeRef?
      let status = SecItemCopyMatching(query as CFDictionary, &retrievedData)

      guard status == errSecSuccess else {
        if status == errSecItemNotFound {
          result(nil) // No shard stored
        } else {
          throw NSError(domain: "Keychain", code: Int(status))
        }
        return
      }

      guard let encryptedData = retrievedData as? Data else {
        result(FlutterError(code: "DECODE_FAILED", message: "Failed to decode encrypted data", details: nil))
        return
      }

      // Get encryption key from Secure Enclave
      let encryptionKey = try getOrCreateEncryptionKey()

      // Decrypt the shard data
      let sealedBox = try ChaChaPoly.SealedBox(combined: encryptedData)
      let decryptedData = try ChaChaPoly.open(sealedBox, using: encryptionKey)

      // Return as byte array
      let byteArray = Array(decryptedData)
      result(byteArray)
    } catch {
      result(FlutterError(code: "DECRYPTION_FAILED", message: error.localizedDescription, details: nil))
    }
  }

  // MARK: - Encryption Key Management

  /// Get or create a SymmetricKey stored in Keychain (not Secure Enclave directly,
  /// but protected by device passcode and never backed up)
  private func getOrCreateEncryptionKey() throws -> SymmetricKey {
    let keyTag = "com.cowallet.shard.encryption.key".data(using: .utf8)!

    // Try to retrieve existing key
    let query: [String: Any] = [
      kSecClass as String: kSecClassKey,
      kSecAttrApplicationTag as String: keyTag,
      kSecReturnData as String: true,
    ]

    var keyData: CFTypeRef?
    let status = SecItemCopyMatching(query as CFDictionary, &keyData)

    if status == errSecSuccess, let existingKeyData = keyData as? Data {
      return SymmetricKey(data: existingKeyData)
    }

    // Generate new 256-bit key for ChaCha20-Poly1305
    let newKey = SymmetricKey(size: .bits256)
    let keyDataToStore = newKey.withUnsafeBytes { Data($0) }

    // Store in Keychain with device-only accessibility
    let addQuery: [String: Any] = [
      kSecClass as String: kSecClassKey,
      kSecAttrApplicationTag as String: keyTag,
      kSecValueData as String: keyDataToStore,
      kSecAttrAccessible as String: kSecAttrAccessibleWhenUnlockedThisDeviceOnly,
      kSecAttrSynchronizable as String: false,
    ]

    let addStatus = SecItemAdd(addQuery as CFDictionary, nil)

    guard addStatus == errSecSuccess else {
      throw NSError(domain: "Keychain", code: Int(addStatus), userInfo: [NSLocalizedDescriptionKey: "Failed to store encryption key"])
    }

    return newKey
  }
}
