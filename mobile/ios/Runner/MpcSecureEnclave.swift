// iOS Secure Enclave Channel Handler
// Handles platform channel calls for SE operations

import Flutter
import CryptoKit
import LocalAuthentication

public class MpcSecureEnclaveHandler: NSObject, FlutterPlugin {
  public static func dummy(methodCall: FlutterMethodCall, result: @escaping FlutterResult) {
    // This method is added to work around a fatal exception when a plugin is
    // registered using generics.
  }

  public static func register(with registrar: FlutterPluginRegistrar) {
    let channel = FlutterMethodChannel(
      name: "com.cowallet.mpc/se",
      binaryMessenger: registrar.messenger()
    )
    let instance = MpcSecureEnclaveHandler()
    registrar.addMethodCallDelegate(instance, channel: channel)
  }

  public func dummyMethodToEnforceBundling() {
    // This method is added to work around a fatal exception when the plugin is
    // registered using generics.
  }

  public func handle(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
    switch call.method {
    case "generateKey":
      generateKey(call, result: result)
    case "getPublicKey":
      getPublicKey(call, result: result)
    case "signWithBiometric":
      signWithBiometric(call, result: result)
    case "isAvailable":
      isAvailable(result: result)
    default:
      result(FlutterMethodNotImplemented)
    }
  }

  // MARK: - Secure Enclave Operations

  /// Generate a new P-256 private key in Secure Enclave
  private func generateKey(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
    guard let args = call.arguments as? [String: Any],
          let keyId = args["keyId"] as? String else {
      result(FlutterError(code: "INVALID_ARGS", message: "keyId is required", details: nil))
      return
    }

    do {
      let privateKey = try SecureEnclave.P256.Signing.PrivateKey()
      let publicKey = privateKey.publicKey

      // Store key in keychain with tag
      let keyTag = "com.cowallet.se.\(keyId)".data(using: .utf8)!
      let privKeyData = privateKey.withUnsafeBytes { Data($0) }

      let query: [String: Any] = [
        kSecClass as String: kSecClassKey,
        kSecAttrApplicationTag as String: keyTag,
        kSecAttrKeyType as String: kSecAttrKeyTypeECSECPr1,
        kSecAttrKeySizeInBits as String: 256,
        kSecValueData as String: privKeyData,
        kSecAttrAccessible as String: kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly,
      ]

      SecItemDelete(query as CFDictionary)
      let status = SecItemAdd(query as CFDictionary, nil)

      guard status == errSecSuccess else {
        throw NSError(domain: "Keychain", code: Int(status))
      }

      // Return public key in compressed format (33 bytes)
      let publicKeyData = publicKey.compressedRepresentation
      let publicKeyBase64 = publicKeyData.base64EncodedString()

      result([
        "publicKey": publicKeyBase64,
        "keyId": keyId,
      ])
    } catch {
      result(FlutterError(code: "GENERATION_FAILED", message: error.localizedDescription, details: nil))
    }
  }

  /// Get public key for a key ID
  private func getPublicKey(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
    guard let args = call.arguments as? [String: Any],
          let keyId = args["keyId"] as? String else {
      result(FlutterError(code: "INVALID_ARGS", message: "keyId is required", details: nil))
      return
    }

    do {
      let keyTag = "com.cowallet.se.\(keyId)".data(using: .utf8)!
      let query: [String: Any] = [
        kSecClass as String: kSecClassKey,
        kSecAttrApplicationTag as String: keyTag,
        kSecReturnRef as String: true,
      ]

      var keyRef: CFTypeRef?
      let status = SecItemCopyMatching(query as CFDictionary, &keyRef)

      guard status == errSecSuccess, let key = keyRef as? SecKey else {
        throw NSError(domain: "Keychain", code: Int(status))
      }

      // Extract public key and compress
      guard let publicKey = SecKeyCopyPublicKey(key) else {
        throw NSError(domain: "Keychain", code: -1)
      }

      // Convert to raw bytes and compress
      var error: Unmanaged<CFError>?
      guard let keyData = SecKeyCopyExternalRepresentation(publicKey, &error) as Data? else {
        throw error?.takeRetainedValue() as Error? ?? NSError(domain: "Keychain", code: -1)
      }

      // Compress public key (65 -> 33 bytes)
      let compressed = compressPublicKey(keyData)
      result(compressed.base64EncodedString())
    } catch {
      result(FlutterError(code: "GET_KEY_FAILED", message: error.localizedDescription, details: nil))
    }
  }

  /// Sign a message with biometric authentication
  private func signWithBiometric(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
    guard let args = call.arguments as? [String: Any],
          let keyId = args["keyId"] as? String,
          let messageBase64 = args["message"] as? String,
          let reason = args["reason"] as? String,
          let messageData = Data(base64Encoded: messageBase64) else {
      result(FlutterError(code: "INVALID_ARGS", message: "keyId, message, and reason are required", details: nil))
      return
    }

    // Authenticate with biometric
    let context = LAContext()
    var error: NSError?

    guard context.canEvaluatePolicy(.deviceOwnerAuthenticationWithBiometrics, error: &error) else {
      let message = error?.localizedDescription ?? "Biometric authentication not available"
      result(FlutterError(code: "BIOMETRIC_UNAVAILABLE", message: message, details: nil))
      return
    }

    context.evaluatePolicy(
      .deviceOwnerAuthenticationWithBiometrics,
      localizedReason: reason
    ) { [weak self] success, authError in
      guard success else {
        let message = authError?.localizedDescription ?? "Authentication failed"
        result(FlutterError(code: "AUTH_FAILED", message: message, details: nil))
        return
      }

      do {
        // Get key from keychain
        let keyTag = "com.cowallet.se.\(keyId)".data(using: .utf8)!
        let query: [String: Any] = [
          kSecClass as String: kSecClassKey,
          kSecAttrApplicationTag as String: keyTag,
          kSecReturnRef as String: true,
        ]

        var keyRef: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &keyRef)

        guard status == errSecSuccess, let key = keyRef as? SecKey else {
          throw NSError(domain: "Keychain", code: Int(status))
        }

        // Sign the message
        var sigError: Unmanaged<CFError>?
        guard let signature = SecKeyCreateSignature(
          key,
          .ecdsaSignatureMessageX962SHA256,
          messageData,
          &sigError
        ) as Data? else {
          throw sigError?.takeRetainedValue() as Error? ?? NSError(domain: "Signing", code: -1)
        }

        result(signature.base64EncodedString())
      } catch {
        result(FlutterError(code: "SIGNING_FAILED", message: error.localizedDescription, details: nil))
      }
    }
  }

  /// Check if Secure Enclave is available
  private func isAvailable(result: @escaping FlutterResult) {
    let context = LAContext()
    var error: NSError?

    // SE is available on iPhone 5s and later
    let hasSecureEnclave = ProcessInfo().isOperatingSystemAtLeast(OperatingSystemVersion(majorVersion: 9, minorVersion: 0, patchVersion: 0))

    result(hasSecureEnclave && context.canEvaluatePolicy(.deviceOwnerAuthenticationWithBiometrics, error: &error))
  }

  // MARK: - Helper Functions

  /// Compress a 65-byte uncompressed public key to 33 bytes
  private func compressPublicKey(_ publicKey: Data) -> Data {
    guard publicKey.count == 65 && publicKey[0] == 0x04 else {
      return publicKey
    }

    let x = publicKey.subdata(in: 1 ..< 33)
    let y = publicKey.subdata(in: 33 ..< 65)

    let isOdd = y[y.count - 1] & 1 == 1
    let prefix = isOdd ? UInt8(0x03) : UInt8(0x02)

    var compressed = Data()
    compressed.append(prefix)
    compressed.append(x)
    return compressed
  }
}
