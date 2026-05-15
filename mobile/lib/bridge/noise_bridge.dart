/// Noise_XX transport encryption bridge.
///
/// Provides a clean Dart API over the flutter_rust_bridge auto-generated
/// FFI functions for Noise protocol operations.
///
/// After modifying `crates/ffi-mobile/src/api.rs`, run:
///   flutter_rust_bridge_codegen generate
///   flutter pub get

import 'dart:typed_data';
import 'frb_generated/api.dart' as frb;

/// Result of a Noise keypair generation.
class NoiseKeypair {
  final List<int> privateKey;
  final List<int> publicKey;
  const NoiseKeypair({required this.privateKey, required this.publicKey});
}

/// Result of a Noise handshake step.
class NoiseHandshakeResult {
  final String sessionId;
  final String messageBase64;
  final bool isReady;
  const NoiseHandshakeResult({
    required this.sessionId,
    required this.messageBase64,
    required this.isReady,
  });
}

/// Generate a new X25519 static keypair for Noise_XX.
Future<NoiseKeypair> noiseGenerateKeypair() async {
  final result = await frb.noiseGenerateKeypair();
  return NoiseKeypair(
    privateKey: result.privateKey,
    publicKey: result.publicKey,
  );
}

/// Create a Noise_XX initiator session and generate the first handshake message.
Future<NoiseHandshakeResult> noiseInitiatorStart({required Uint8List staticPrivateKey}) async {
  final result = await frb.noiseInitiatorStart(staticPrivateKey: staticPrivateKey);
  return NoiseHandshakeResult(
    sessionId: '',  // session ID is managed internally by Rust; use the returned message
    messageBase64: result.messageBase64,
    isReady: result.isReady,
  );
}

/// Process the server's handshake response and generate the final message.
Future<NoiseHandshakeResult> noiseInitiatorFinish({
  required String sessionId,
  required String serverMessageBase64,
}) async {
  final result = await frb.noiseInitiatorFinish(
    sessionId: sessionId,
    serverMessageBase64: serverMessageBase64,
  );
  return NoiseHandshakeResult(
    sessionId: sessionId,
    messageBase64: result.messageBase64,
    isReady: result.isReady,
  );
}

/// Encrypt plaintext using the established Noise session.
/// Returns base64-encoded ciphertext.
Future<String> noiseEncrypt({required String sessionId, required Uint8List plaintext}) async {
  return await frb.noiseEncrypt(sessionId: sessionId, plaintext: plaintext);
}

/// Decrypt ciphertext using the established Noise session.
/// Returns decrypted plaintext bytes.
Future<List<int>> noiseDecrypt({
  required String sessionId,
  required String ciphertextBase64,
}) async {
  return await frb.noiseDecrypt(
    sessionId: sessionId,
    ciphertextBase64: ciphertextBase64,
  );
}

/// Get the remote peer's static public key after handshake completes.
Future<List<int>> noiseGetRemotePublicKey({required String sessionId}) async {
  return await frb.noiseGetRemotePublicKey(sessionId: sessionId);
}

/// Destroy a Noise session and free its resources.
Future<void> noiseSessionDestroy({required String sessionId}) async {
  await frb.noiseSessionDestroy(sessionId: sessionId);
}
