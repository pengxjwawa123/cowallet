import Flutter
import Security

public class CloudBackupHandler: NSObject, FlutterPlugin {
  private static let service = "com.cowallet.cloud_backup"

  public static func register(with registrar: FlutterPluginRegistrar) {
    let channel = FlutterMethodChannel(
      name: "com.cowallet/cloud_backup",
      binaryMessenger: registrar.messenger()
    )
    let instance = CloudBackupHandler()
    registrar.addMethodCallDelegate(instance, channel: channel)
  }

  public func handle(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
    switch call.method {
    case "isAvailable":
      result(true)
    case "store":
      store(call, result: result)
    case "retrieve":
      retrieve(call, result: result)
    case "delete":
      delete(call, result: result)
    default:
      result(FlutterMethodNotImplemented)
    }
  }

  private func store(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
    guard let args = call.arguments as? [String: Any],
          let key = args["key"] as? String,
          let data = args["data"] as? String else {
      result(FlutterError(code: "INVALID_ARGS", message: "key and data are required", details: nil))
      return
    }

    guard let valueData = data.data(using: .utf8) else {
      result(FlutterError(code: "ENCODE_FAILED", message: "Failed to encode data", details: nil))
      return
    }

    let deleteQuery: [String: Any] = [
      kSecClass as String: kSecClassGenericPassword,
      kSecAttrAccount as String: key,
      kSecAttrService as String: CloudBackupHandler.service,
      kSecAttrSynchronizable as String: kSecAttrSynchronizableAny,
    ]
    SecItemDelete(deleteQuery as CFDictionary)

    let addQuery: [String: Any] = [
      kSecClass as String: kSecClassGenericPassword,
      kSecAttrAccount as String: key,
      kSecAttrService as String: CloudBackupHandler.service,
      kSecValueData as String: valueData,
      kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlock,
      kSecAttrSynchronizable as String: true,
    ]

    let status = SecItemAdd(addQuery as CFDictionary, nil)
    if status == errSecSuccess {
      result(nil)
    } else {
      result(FlutterError(code: "STORE_FAILED", message: "Keychain store failed: \(status)", details: nil))
    }
  }

  private func retrieve(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
    guard let args = call.arguments as? [String: Any],
          let key = args["key"] as? String else {
      result(FlutterError(code: "INVALID_ARGS", message: "key is required", details: nil))
      return
    }

    let query: [String: Any] = [
      kSecClass as String: kSecClassGenericPassword,
      kSecAttrAccount as String: key,
      kSecAttrService as String: CloudBackupHandler.service,
      kSecAttrSynchronizable as String: true,
      kSecReturnData as String: true,
    ]

    var retrievedData: CFTypeRef?
    let status = SecItemCopyMatching(query as CFDictionary, &retrievedData)

    if status == errSecItemNotFound {
      result(nil)
    } else if status == errSecSuccess, let data = retrievedData as? Data,
              let value = String(data: data, encoding: .utf8) {
      result(value)
    } else {
      result(FlutterError(code: "RETRIEVE_FAILED", message: "Keychain retrieve failed: \(status)", details: nil))
    }
  }

  private func delete(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
    guard let args = call.arguments as? [String: Any],
          let key = args["key"] as? String else {
      result(FlutterError(code: "INVALID_ARGS", message: "key is required", details: nil))
      return
    }

    let query: [String: Any] = [
      kSecClass as String: kSecClassGenericPassword,
      kSecAttrAccount as String: key,
      kSecAttrService as String: CloudBackupHandler.service,
      kSecAttrSynchronizable as String: true,
    ]

    let status = SecItemDelete(query as CFDictionary)
    if status == errSecSuccess || status == errSecItemNotFound {
      result(nil)
    } else {
      result(FlutterError(code: "DELETE_FAILED", message: "Keychain delete failed: \(status)", details: nil))
    }
  }
}
