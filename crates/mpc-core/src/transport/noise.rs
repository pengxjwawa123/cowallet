use crate::errors::{MpcError, Result};

/// Noise_XX encrypted channel for MPC message transport.
///
/// Provides authenticated encryption with forward secrecy between
/// two MPC parties. Uses the `snow` crate (Noise Protocol Framework).
///
/// Handshake pattern: XX (mutual authentication, no pre-shared keys)
/// Cipher: ChaChaPoly
/// DH: 25519
pub struct NoiseChannel {
    // TODO: snow::TransportState after handshake completes
    _state: (),
}

impl NoiseChannel {
    /// Initiate a Noise_XX handshake as the initiator.
    pub fn initiate(_local_keypair: &[u8]) -> Result<(Self, Vec<u8>)> {
        // TODO: Initialize snow builder with Noise_XX pattern
        // let builder = snow::Builder::new("Noise_XX_25519_ChaChaPoly_SHA256".parse().unwrap());
        // let keypair = builder.generate_keypair().unwrap();
        // let mut handshake = builder.local_private_key(&keypair.private).build_initiator().unwrap();
        // let mut buf = vec![0u8; 65535];
        // let len = handshake.write_message(&[], &mut buf).unwrap();
        Err(MpcError::Transport("not yet implemented".into()))
    }

    /// Respond to a Noise_XX handshake as the responder.
    pub fn respond(_local_keypair: &[u8], _initiator_msg: &[u8]) -> Result<(Self, Vec<u8>)> {
        Err(MpcError::Transport("not yet implemented".into()))
    }

    /// Encrypt a message for the peer.
    pub fn encrypt(&mut self, _plaintext: &[u8]) -> Result<Vec<u8>> {
        Err(MpcError::Transport("not yet implemented".into()))
    }

    /// Decrypt a message from the peer.
    pub fn decrypt(&mut self, _ciphertext: &[u8]) -> Result<Vec<u8>> {
        Err(MpcError::Transport("not yet implemented".into()))
    }
}
