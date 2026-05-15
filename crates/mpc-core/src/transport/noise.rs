use crate::errors::{MpcError, Result};
use snow::{HandshakeState, TransportState};
use zeroize::Zeroize;

/// Noise_XX encrypted channel for MPC message transport.
///
/// Provides authenticated encryption with forward secrecy between
/// two MPC parties. Uses the `snow` crate (Noise Protocol Framework).
///
/// Handshake pattern: XX (mutual authentication, no pre-shared keys)
/// Cipher: ChaChaPoly
/// DH: X25519
/// Hash: SHA256
pub struct NoiseChannel {
    state: ChannelState,
    peer_public_key: Option<[u8; 32]>,
}

enum ChannelState {
    HandshakeInitiator(HandshakeState),
    HandshakeResponder(HandshakeState),
    Transport(TransportState),
    Temporary,
}

impl Zeroize for ChannelState {
    fn zeroize(&mut self) {
        // HandshakeState and TransportState handle their own zeroization
        // via snow's internal zero-on-drop guarantees
    }
}

impl Drop for NoiseChannel {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl Zeroize for NoiseChannel {
    fn zeroize(&mut self) {
        self.peer_public_key.zeroize();
        // Replace state with a dummy to clear it and trigger drop of inner state
        self.state = ChannelState::Temporary;
    }
}

/// Noise protocol parameters: XX pattern with 25519, ChaChaPoly, SHA256
const NOISE_PARAMS: &str = "Noise_XX_25519_ChaChaPoly_SHA256";

// ---------------------------------------------------------------------------
// NoiseSession — persistent static-key based Noise_XX session for WebSocket
// ---------------------------------------------------------------------------

/// Noise_XX session with persistent static keys for WebSocket MPC transport.
///
/// Unlike `NoiseChannel` which generates ephemeral keys per handshake,
/// `NoiseSession` accepts a pre-existing static key (stored per device/server)
/// and exposes a step-based handshake API suitable for WebSocket message flow.
///
/// Usage (Initiator / device side):
/// ```ignore
/// let session = NoiseSession::new_initiator(&my_static_private_key)?;
/// let msg1 = session.handshake_step(&[])?;       // -> e
/// // send msg1, receive msg2
/// let msg3 = session.handshake_step(&msg2)?;     // <- e,ee,s,es  -> s,se
/// // send msg3, handshake complete
/// assert!(session.is_transport_ready());
/// ```
///
/// Usage (Responder / server side):
/// ```ignore
/// let session = NoiseSession::new_responder(&server_static_private_key)?;
/// // receive msg1
/// let msg2 = session.handshake_step(&msg1)?;     // <- e  -> e,ee,s,es
/// // send msg2, receive msg3
/// let empty = session.handshake_step(&msg3)?;    // <- s,se  (returns empty, transition to transport)
/// assert!(session.is_transport_ready());
/// ```
pub struct NoiseSession {
    state: SessionState,
    remote_static: Option<Vec<u8>>,
}

enum SessionState {
    /// Initiator before sending first message (-> e)
    InitiatorStart(Box<snow::HandshakeState>),
    /// Initiator waiting for responder's message, then sends final (-> s, se)
    InitiatorWaitResp(Box<snow::HandshakeState>),
    /// Responder waiting for initiator's first message
    ResponderStart(Box<snow::HandshakeState>),
    /// Responder waiting for initiator's final message (-> s, se)
    ResponderWaitFinal(Box<snow::HandshakeState>),
    /// Handshake complete, transport encryption active
    Transport(Box<snow::TransportState>),
    /// Placeholder during state transitions
    Poisoned,
}

impl NoiseSession {
    /// Create an initiator session with a pre-existing static private key (32 bytes X25519).
    pub fn new_initiator(local_static_key: &[u8]) -> Result<Self> {
        let handshake = snow::Builder::new(NOISE_PARAMS.parse().expect("valid noise params"))
            .local_private_key(local_static_key)
            .build_initiator()
            .map_err(|e| MpcError::Transport(format!("failed to build initiator: {}", e)))?;

        Ok(Self {
            state: SessionState::InitiatorStart(Box::new(handshake)),
            remote_static: None,
        })
    }

    /// Create a responder session with a pre-existing static private key (32 bytes X25519).
    pub fn new_responder(local_static_key: &[u8]) -> Result<Self> {
        let handshake = snow::Builder::new(NOISE_PARAMS.parse().expect("valid noise params"))
            .local_private_key(local_static_key)
            .build_responder()
            .map_err(|e| MpcError::Transport(format!("failed to build responder: {}", e)))?;

        Ok(Self {
            state: SessionState::ResponderStart(Box::new(handshake)),
            remote_static: None,
        })
    }

    /// Process one handshake step. The semantics depend on the current state:
    ///
    /// **Initiator flow** (call twice):
    /// 1. `handshake_step(&[])` — generates first message (-> e). Pass empty slice.
    /// 2. `handshake_step(&responder_msg)` — processes responder, generates final (-> s,se).
    ///    After this call, transport is ready.
    ///
    /// **Responder flow** (call twice):
    /// 1. `handshake_step(&initiator_msg1)` — processes initiator's e, generates response (<- e,ee,s,es).
    /// 2. `handshake_step(&initiator_msg3)` — processes initiator's final (-> s,se).
    ///    Returns empty vec. After this call, transport is ready.
    pub fn handshake_step(&mut self, incoming: &[u8]) -> Result<Vec<u8>> {
        match std::mem::replace(&mut self.state, SessionState::Poisoned) {
            SessionState::InitiatorStart(mut hs) => {
                // Initiator sends first message: -> e
                let mut buf = vec![0u8; 65535];
                let len = hs.write_message(&[], &mut buf)
                    .map_err(|e| MpcError::Transport(format!("handshake write (-> e) failed: {}", e)))?;
                buf.truncate(len);
                self.state = SessionState::InitiatorWaitResp(hs);
                Ok(buf)
            }
            SessionState::InitiatorWaitResp(mut hs) => {
                // Process responder's message: <- e, ee, s, es
                let mut payload_buf = vec![0u8; 65535];
                hs.read_message(incoming, &mut payload_buf)
                    .map_err(|e| MpcError::Transport(format!("handshake read (<- e,ee,s,es) failed: {}", e)))?;

                // Extract remote static key
                if let Some(rs) = hs.get_remote_static() {
                    self.remote_static = Some(rs.to_vec());
                }

                // Write final initiator message: -> s, se
                let mut buf = vec![0u8; 65535];
                let len = hs.write_message(&[], &mut buf)
                    .map_err(|e| MpcError::Transport(format!("handshake write (-> s,se) failed: {}", e)))?;
                buf.truncate(len);

                // Transition to transport mode
                let transport = hs.into_transport_mode()
                    .map_err(|e| MpcError::Transport(format!("transport transition failed: {}", e)))?;
                self.state = SessionState::Transport(Box::new(transport));
                Ok(buf)
            }
            SessionState::ResponderStart(mut hs) => {
                // Process initiator's first message and send response
                let mut payload_buf = vec![0u8; 65535];
                hs.read_message(incoming, &mut payload_buf)
                    .map_err(|e| MpcError::Transport(format!("handshake read (-> e) failed: {}", e)))?;

                // Write responder's message: <- e, ee, s, es
                let mut buf = vec![0u8; 65535];
                let len = hs.write_message(&[], &mut buf)
                    .map_err(|e| MpcError::Transport(format!("handshake write (<- e,ee,s,es) failed: {}", e)))?;
                buf.truncate(len);

                self.state = SessionState::ResponderWaitFinal(hs);
                Ok(buf)
            }
            SessionState::ResponderWaitFinal(mut hs) => {
                // Process initiator's final message: -> s, se
                let mut payload_buf = vec![0u8; 65535];
                hs.read_message(incoming, &mut payload_buf)
                    .map_err(|e| MpcError::Transport(format!("handshake read (-> s,se) failed: {}", e)))?;

                // Extract remote static key
                if let Some(rs) = hs.get_remote_static() {
                    self.remote_static = Some(rs.to_vec());
                }

                // Transition to transport mode
                let transport = hs.into_transport_mode()
                    .map_err(|e| MpcError::Transport(format!("transport transition failed: {}", e)))?;
                self.state = SessionState::Transport(Box::new(transport));

                // Return empty — no message to send after final read
                Ok(Vec::new())
            }
            SessionState::Transport(_) => {
                Err(MpcError::Transport("handshake already complete".into()))
            }
            SessionState::Poisoned => {
                Err(MpcError::Transport("session in poisoned state (prior error)".into()))
            }
        }
    }

    /// Returns true if the handshake is complete and the session is ready for encrypt/decrypt.
    pub fn is_transport_ready(&self) -> bool {
        matches!(self.state, SessionState::Transport(_))
    }

    /// Encrypt a plaintext message. Only valid after handshake completes.
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let transport = match &mut self.state {
            SessionState::Transport(t) => t,
            _ => return Err(MpcError::Transport("cannot encrypt: handshake not complete".into())),
        };
        let mut buf = vec![0u8; plaintext.len() + 16]; // +16 for AEAD tag
        let len = transport.write_message(plaintext, &mut buf)
            .map_err(|e| MpcError::Transport(format!("encryption failed: {}", e)))?;
        buf.truncate(len);
        Ok(buf)
    }

    /// Decrypt a ciphertext message. Only valid after handshake completes.
    pub fn decrypt(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        let transport = match &mut self.state {
            SessionState::Transport(t) => t,
            _ => return Err(MpcError::Transport("cannot decrypt: handshake not complete".into())),
        };
        let mut buf = vec![0u8; ciphertext.len()];
        let len = transport.read_message(ciphertext, &mut buf)
            .map_err(|e| MpcError::Transport(format!("decryption failed: {}", e)))?;
        buf.truncate(len);
        Ok(buf)
    }

    /// Get the remote peer's static public key after handshake completes.
    /// Returns `None` if handshake is not yet finished.
    pub fn remote_static_key(&self) -> Option<&[u8]> {
        self.remote_static.as_deref()
    }
}

/// Generate a new X25519 static keypair for Noise_XX.
/// Returns (private_key[32], public_key[32]).
pub fn generate_keypair() -> Result<(Vec<u8>, Vec<u8>)> {
    let builder = snow::Builder::new(NOISE_PARAMS.parse().expect("valid noise params"));
    let keypair = builder.generate_keypair()
        .map_err(|e| MpcError::Transport(format!("keypair generation failed: {}", e)))?;
    Ok((keypair.private, keypair.public))
}

// ---------------------------------------------------------------------------
// NoiseChannel — ephemeral-key based (original implementation)
// ---------------------------------------------------------------------------

impl NoiseChannel {
    /// Create a new Noise channel builder.
    fn builder() -> snow::Builder<'static> {
        snow::Builder::new(NOISE_PARAMS.parse().expect("valid Noise parameters"))
    }

    /// Initiate a Noise_XX handshake as the initiator.
    ///
    /// Returns (channel, first_handshake_message) to send to the responder.
    pub fn initiate() -> Result<(Self, Vec<u8>)> {
        let builder = Self::builder();
        let keypair = builder
            .generate_keypair()
            .map_err(|e| MpcError::Transport(format!("failed to generate keypair: {}", e)))?;
        let mut handshake = Self::builder()
            .local_private_key(&keypair.private)
            .build_initiator()
            .map_err(|e| MpcError::Transport(format!("failed to build initiator: {}", e)))?;

        // First handshake message: -> e
        let mut buf = vec![0u8; 65535];
        let len = handshake
            .write_message(&[], &mut buf)
            .map_err(|e| MpcError::Transport(format!("handshake write failed: {}", e)))?;
        buf.truncate(len);

        Ok((
            Self {
                state: ChannelState::HandshakeInitiator(handshake),
                peer_public_key: None,
            },
            buf,
        ))
    }

    /// Respond to a Noise_XX handshake as the responder.
    ///
    /// Receives the initiator's first message, returns (channel, response_message).
    pub fn respond(initiator_msg: &[u8]) -> Result<(Self, Vec<u8>)> {
        let builder = Self::builder();
        let keypair = builder
            .generate_keypair()
            .map_err(|e| MpcError::Transport(format!("failed to generate keypair: {}", e)))?;
        let mut handshake = Self::builder()
            .local_private_key(&keypair.private)
            .build_responder()
            .map_err(|e| MpcError::Transport(format!("failed to build responder: {}", e)))?;

        // Process initiator's first message
        handshake
            .read_message(initiator_msg, &mut [])
            .map_err(|e| MpcError::Transport(format!("failed to read initiator message: {}", e)))?;

        // Send response: <- e, ee, s, es
        let mut buf = vec![0u8; 65535];
        let len = handshake
            .write_message(&[], &mut buf)
            .map_err(|e| MpcError::Transport(format!("handshake write failed: {}", e)))?;
        buf.truncate(len);

        Ok((
            Self {
                state: ChannelState::HandshakeResponder(handshake),
                peer_public_key: None,
            },
            buf,
        ))
    }

    /// Complete the handshake as initiator after receiving responder's message.
    ///
    /// This processes the responder's message and generates the final initiator message.
    /// Returns the final message to send to the responder.
    /// After calling this, the channel is ready for transport encryption.
    pub fn complete_handshake_initiator(&mut self, responder_msg: &[u8]) -> Result<Vec<u8>> {
        // First process the responder's message
        let (peer_pub, final_msg) = {
            let handshake = match &mut self.state {
                ChannelState::HandshakeInitiator(h) => h,
                _ => return Err(MpcError::Transport("not in initiator handshake state".into())),
            };

            // Process responder's message: <- e, ee, s, es
            handshake
                .read_message(responder_msg, &mut [])
                .map_err(|e| MpcError::Transport(format!("failed to read responder message: {}", e)))?;

            // Get peer's static public key from handshake
            let pubkey = handshake
                .get_remote_static()
                .ok_or_else(|| MpcError::Transport("no remote public key after handshake".into()))?
                .to_vec();

            // Write the final initiator message (-> s, se)
            let mut buf = vec![0u8; 65535];
            let len = handshake
                .write_message(&[], &mut buf)
                .map_err(|e| MpcError::Transport(format!("handshake write failed: {}", e)))?;
            buf.truncate(len);

            (pubkey, buf)
        };

        let mut key = [0u8; 32];
        key.copy_from_slice(&peer_pub);
        self.peer_public_key = Some(key);

        // Now transition to transport mode
        let handshake = match std::mem::replace(&mut self.state, ChannelState::Temporary) {
            ChannelState::HandshakeInitiator(h) => h,
            _ => return Err(MpcError::Transport("not in initiator handshake state".into())),
        };

        let transport = handshake
            .into_transport_mode()
            .map_err(|e| MpcError::Transport(format!("failed to enter transport mode: {}", e)))?;

        self.state = ChannelState::Transport(transport);
        Ok(final_msg)
    }

    /// Complete the handshake as responder after receiving initiator's final message.
    ///
    /// After calling this, the channel is ready for transport encryption.
    pub fn complete_handshake_responder(&mut self, initiator_final_msg: &[u8]) -> Result<()> {
        // First process the message while we still have the handshake state
        let peer_pub = {
            let handshake = match &mut self.state {
                ChannelState::HandshakeResponder(h) => h,
                _ => return Err(MpcError::Transport("not in responder handshake state".into())),
            };

            // Process initiator's final message: -> s, se
            handshake
                .read_message(initiator_final_msg, &mut [])
                .map_err(|e| MpcError::Transport(format!("failed to read final message: {}", e)))?;

            // Get peer's static public key from handshake
            handshake
                .get_remote_static()
                .ok_or_else(|| MpcError::Transport("no remote public key after handshake".into()))?
                .to_vec()
        };

        let mut key = [0u8; 32];
        key.copy_from_slice(&peer_pub);
        self.peer_public_key = Some(key);

        // Now transition to transport mode
        let handshake = match std::mem::replace(&mut self.state, ChannelState::Temporary) {
            ChannelState::HandshakeResponder(h) => h,
            _ => return Err(MpcError::Transport("not in responder handshake state".into())),
        };

        let transport = handshake
            .into_transport_mode()
            .map_err(|e| MpcError::Transport(format!("failed to enter transport mode: {}", e)))?;

        self.state = ChannelState::Transport(transport);
        Ok(())
    }

    /// Check if the handshake is complete and channel is ready for transport.
    pub fn is_ready(&self) -> bool {
        matches!(self.state, ChannelState::Transport(_))
    }

    /// Get the peer's public key if handshake is complete.
    pub fn peer_public_key(&self) -> Option<&[u8; 32]> {
        self.peer_public_key.as_ref()
    }

    /// Encrypt a message for the peer.
    ///
    /// In transport mode, each message is encrypted with an incrementing nonce.
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let transport = match &mut self.state {
            ChannelState::Transport(t) => t,
            _ => return Err(MpcError::Transport("handshake not complete".into())),
        };

        // snow::TransportState::write_message returns ciphertext with MAC appended
        let mut buf = vec![0u8; plaintext.len() + 16]; // +16 for ChaChaPoly tag
        let len = transport
            .write_message(plaintext, &mut buf)
            .map_err(|e| MpcError::Transport(format!("encryption failed: {}", e)))?;
        buf.truncate(len);
        Ok(buf)
    }

    /// Decrypt a message from the peer.
    pub fn decrypt(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        let transport = match &mut self.state {
            ChannelState::Transport(t) => t,
            _ => return Err(MpcError::Transport("handshake not complete".into())),
        };

        let mut buf = vec![0u8; ciphertext.len()];
        let len = transport
            .read_message(ciphertext, &mut buf)
            .map_err(|e| MpcError::Transport(format!("decryption failed: {}", e)))?;
        buf.truncate(len);
        Ok(buf)
    }
}

/// Convenience function: perform a full Noise_XX handshake between two parties.
/// Returns (initiator_channel, responder_channel) ready for transport.
pub fn handshake_pair() -> Result<(NoiseChannel, NoiseChannel)> {
    // 1. Initiator sends first message (e)
    let (mut initiator, msg1) = NoiseChannel::initiate()?;

    // 2. Responder receives msg1 and sends response (e, ee, s, es)
    let (mut responder, msg2) = NoiseChannel::respond(&msg1)?;

    // 3. Initiator receives responder's message and sends final message (s, se)
    let msg3 = initiator.complete_handshake_initiator(&msg2)?;

    // 4. Responder receives final message
    responder.complete_handshake_responder(&msg3)?;

    assert!(initiator.is_ready());
    assert!(responder.is_ready());

    // Note: In Noise_XX with no pre-configured static keys, each side generates ephemeral keys
    // The "remote static" in this context is the ephemeral key used during handshake

    Ok((initiator, responder))
}

#[cfg(test)]
mod tests {
    use super::*;
    use zeroize::Zeroize;

    // -----------------------------------------------------------------------
    // NoiseChannel (ephemeral key) tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_handshake_completes() {
        let (mut alice, mut bob) = handshake_pair().unwrap();
        assert!(alice.is_ready());
        assert!(bob.is_ready());
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let (mut alice, mut bob) = handshake_pair().unwrap();

        // Alice -> Bob
        let plaintext = b"Hello, MPC world!";
        let ciphertext = alice.encrypt(plaintext).unwrap();
        let decrypted = bob.decrypt(&ciphertext).unwrap();
        assert_eq!(decrypted, plaintext);

        // Bob -> Alice
        let response = b"Response from Bob";
        let ciphertext2 = bob.encrypt(response).unwrap();
        let decrypted2 = alice.decrypt(&ciphertext2).unwrap();
        assert_eq!(decrypted2, response);
    }

    #[test]
    fn test_multiple_messages() {
        let (mut alice, mut bob) = handshake_pair().unwrap();

        for i in 0..10 {
            let msg = format!("Message {}", i);
            let ciphertext = alice.encrypt(msg.as_bytes()).unwrap();
            let decrypted = bob.decrypt(&ciphertext).unwrap();
            assert_eq!(decrypted, msg.as_bytes());
        }
    }

    #[test]
    fn test_large_message() {
        let (mut alice, mut bob) = handshake_pair().unwrap();

        // 4KB message
        let large_msg = vec![0x42u8; 4096];
        let ciphertext = alice.encrypt(&large_msg).unwrap();
        let decrypted = bob.decrypt(&ciphertext).unwrap();
        assert_eq!(decrypted, large_msg);
    }

    #[test]
    fn test_decrypt_fails_with_wrong_data() {
        let (mut alice, mut bob) = handshake_pair().unwrap();

        let plaintext = b"Hello";
        let mut ciphertext = alice.encrypt(plaintext).unwrap();

        // Corrupt the ciphertext
        ciphertext[0] ^= 0xFF;

        let result = bob.decrypt(&ciphertext);
        assert!(result.is_err());
    }

    #[test]
    fn test_encrypt_fails_before_handshake() {
        let (mut alice, _) = NoiseChannel::initiate().unwrap();
        assert!(!alice.is_ready());

        let result = alice.encrypt(b"too early");
        assert!(result.is_err());
    }

    #[test]
    fn test_zeroize() {
        let (mut alice, mut bob) = handshake_pair().unwrap();

        // Use the channel
        let ciphertext = alice.encrypt(b"test").unwrap();
        let _ = bob.decrypt(&ciphertext).unwrap();

        // Zeroize
        alice.zeroize();
        bob.zeroize();

        // Channel should no longer be usable
        assert!(!alice.is_ready());
        assert!(!bob.is_ready());
    }

    // -----------------------------------------------------------------------
    // NoiseSession (persistent static key) tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_noise_session_handshake_and_transport() {
        // Generate persistent static keys for both parties
        let (initiator_priv, _initiator_pub) = generate_keypair().unwrap();
        let (responder_priv, responder_pub) = generate_keypair().unwrap();

        // Create sessions
        let mut initiator = NoiseSession::new_initiator(&initiator_priv).unwrap();
        let mut responder = NoiseSession::new_responder(&responder_priv).unwrap();

        // Step 1: Initiator generates first message (-> e)
        let msg1 = initiator.handshake_step(&[]).unwrap();
        assert!(!initiator.is_transport_ready());
        assert!(!msg1.is_empty());

        // Step 2: Responder processes msg1 and generates response (<- e, ee, s, es)
        let msg2 = responder.handshake_step(&msg1).unwrap();
        assert!(!responder.is_transport_ready());
        assert!(!msg2.is_empty());

        // Step 3: Initiator processes msg2 and generates final (-> s, se)
        let msg3 = initiator.handshake_step(&msg2).unwrap();
        assert!(initiator.is_transport_ready());
        assert!(!msg3.is_empty());

        // Step 4: Responder processes msg3 (completes handshake)
        let msg4 = responder.handshake_step(&msg3).unwrap();
        assert!(responder.is_transport_ready());
        assert!(msg4.is_empty()); // No message to send after final read

        // Verify remote static keys
        let initiator_sees_remote = initiator.remote_static_key().unwrap();
        assert_eq!(initiator_sees_remote, &responder_pub);
    }

    #[test]
    fn test_noise_session_encrypt_decrypt_bidirectional() {
        let (init_priv, _) = generate_keypair().unwrap();
        let (resp_priv, _) = generate_keypair().unwrap();

        let mut initiator = NoiseSession::new_initiator(&init_priv).unwrap();
        let mut responder = NoiseSession::new_responder(&resp_priv).unwrap();

        // Complete handshake
        let msg1 = initiator.handshake_step(&[]).unwrap();
        let msg2 = responder.handshake_step(&msg1).unwrap();
        let msg3 = initiator.handshake_step(&msg2).unwrap();
        let _ = responder.handshake_step(&msg3).unwrap();

        // Initiator -> Responder
        let plaintext = b"MPC DKG round 1 commitment data";
        let ciphertext = initiator.encrypt(plaintext).unwrap();
        assert_ne!(ciphertext, plaintext.to_vec());
        let decrypted = responder.decrypt(&ciphertext).unwrap();
        assert_eq!(decrypted, plaintext);

        // Responder -> Initiator
        let response = b"MPC DKG round 1 response from server";
        let ct2 = responder.encrypt(response).unwrap();
        let dec2 = initiator.decrypt(&ct2).unwrap();
        assert_eq!(dec2, response);
    }

    #[test]
    fn test_noise_session_multiple_messages_sequential() {
        let (init_priv, _) = generate_keypair().unwrap();
        let (resp_priv, _) = generate_keypair().unwrap();

        let mut initiator = NoiseSession::new_initiator(&init_priv).unwrap();
        let mut responder = NoiseSession::new_responder(&resp_priv).unwrap();

        // Complete handshake
        let msg1 = initiator.handshake_step(&[]).unwrap();
        let msg2 = responder.handshake_step(&msg1).unwrap();
        let msg3 = initiator.handshake_step(&msg2).unwrap();
        let _ = responder.handshake_step(&msg3).unwrap();

        // Send 20 messages in alternating directions (simulating MPC rounds)
        for i in 0..20 {
            let payload = format!("round {} payload with data {}", i, "x".repeat(100));
            if i % 2 == 0 {
                let ct = initiator.encrypt(payload.as_bytes()).unwrap();
                let pt = responder.decrypt(&ct).unwrap();
                assert_eq!(pt, payload.as_bytes());
            } else {
                let ct = responder.encrypt(payload.as_bytes()).unwrap();
                let pt = initiator.decrypt(&ct).unwrap();
                assert_eq!(pt, payload.as_bytes());
            }
        }
    }

    #[test]
    fn test_noise_session_encrypt_before_handshake_fails() {
        let (init_priv, _) = generate_keypair().unwrap();
        let mut session = NoiseSession::new_initiator(&init_priv).unwrap();

        let result = session.encrypt(b"too early");
        assert!(result.is_err());
    }

    #[test]
    fn test_noise_session_tampered_ciphertext_fails() {
        let (init_priv, _) = generate_keypair().unwrap();
        let (resp_priv, _) = generate_keypair().unwrap();

        let mut initiator = NoiseSession::new_initiator(&init_priv).unwrap();
        let mut responder = NoiseSession::new_responder(&resp_priv).unwrap();

        let msg1 = initiator.handshake_step(&[]).unwrap();
        let msg2 = responder.handshake_step(&msg1).unwrap();
        let msg3 = initiator.handshake_step(&msg2).unwrap();
        let _ = responder.handshake_step(&msg3).unwrap();

        let ct = initiator.encrypt(b"secret message").unwrap();
        let mut tampered = ct.clone();
        tampered[0] ^= 0xFF;

        let result = responder.decrypt(&tampered);
        assert!(result.is_err());
    }

    #[test]
    fn test_noise_session_remote_static_keys_match() {
        let (init_priv, init_pub) = generate_keypair().unwrap();
        let (resp_priv, resp_pub) = generate_keypair().unwrap();

        let mut initiator = NoiseSession::new_initiator(&init_priv).unwrap();
        let mut responder = NoiseSession::new_responder(&resp_priv).unwrap();

        let msg1 = initiator.handshake_step(&[]).unwrap();
        let msg2 = responder.handshake_step(&msg1).unwrap();
        let msg3 = initiator.handshake_step(&msg2).unwrap();
        let _ = responder.handshake_step(&msg3).unwrap();

        // Initiator sees responder's static key
        assert_eq!(initiator.remote_static_key().unwrap(), &resp_pub);
        // Responder sees initiator's static key
        assert_eq!(responder.remote_static_key().unwrap(), &init_pub);
    }

    #[test]
    fn test_generate_keypair_produces_valid_keys() {
        let (priv_key, pub_key) = generate_keypair().unwrap();
        assert_eq!(priv_key.len(), 32);
        assert_eq!(pub_key.len(), 32);

        // Keys should be different
        assert_ne!(priv_key, pub_key);

        // Should be usable to create sessions
        let session = NoiseSession::new_initiator(&priv_key);
        assert!(session.is_ok());
    }
}
