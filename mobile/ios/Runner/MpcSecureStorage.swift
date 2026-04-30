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
}
