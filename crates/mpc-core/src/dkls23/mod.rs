pub mod dkg;
pub mod presign;
pub mod protocol;
pub mod reshare;
pub mod sign;

use crate::security::SecureVec;
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Index identifying a party in the threshold scheme (0, 1, or 2 for 2-of-3).
pub type PartyIndex = u16;

/// A key share held by one party after DKG.
///
/// Contains the party's secret share of the private key and the joint public key.
/// The secret component is memory-locked using mlock and zeroized on drop.
#[derive(Clone, ZeroizeOnDrop)]
pub struct KeyShare {
    /// This party's index.
    pub party: PartyIndex,

    /// Threshold: minimum parties needed to sign.
    pub threshold: u16,

    /// Total number of parties.
    pub total_parties: u16,

    /// Secret share bytes (the sensitive part) - memory-locked and zeroized on drop.
    pub secret_share: SecureVec,

    /// The joint public key (not sensitive).
    pub public_key: Vec<u8>,

    /// Paillier public key of this party's signing counterpart.
    /// Device stores the server's Paillier pk; Server stores device's Paillier pk.
    /// None for the backup shard (Party 2) which doesn't participate in signing.
    #[zeroize(skip)]
    pub paillier_pk: Option<Vec<u8>>,
}

impl Zeroize for KeyShare {
    fn zeroize(&mut self) {
        // SecureVec handles its own zeroization
        // We just clear the non-sensitive fields
        self.party = 0;
        self.threshold = 0;
        self.total_parties = 0;
    }
}

impl KeyShare {
    /// Derive the Ethereum address from the joint public key.
    ///
    /// Handles both compressed (33 bytes, 0x02/0x03 prefix) and
    /// uncompressed (65 bytes, 0x04 prefix) SEC1 encodings.
    /// Ethereum address = keccak256(uncompressed_x_y)[12..32].
    pub fn eth_address(&self) -> [u8; 20] {
        use k256::elliptic_curve::sec1::{FromEncodedPoint, ToEncodedPoint};
        use k256::{AffinePoint, EncodedPoint};
        use sha3::{Digest, Keccak256};

        let uncompressed_xy = if self.public_key.len() == 65 && self.public_key[0] == 0x04 {
            // Already uncompressed: skip the 0x04 prefix
            self.public_key[1..].to_vec()
        } else if (self.public_key.len() == 33)
            && (self.public_key[0] == 0x02 || self.public_key[0] == 0x03)
        {
            // Compressed: decompress via k256
            let encoded = EncodedPoint::from_bytes(&self.public_key)
                .expect("invalid compressed pubkey");
            let point = AffinePoint::from_encoded_point(&encoded)
                .expect("decompression failed");
            let uncompressed = point.to_encoded_point(false);
            // uncompressed is 65 bytes (0x04 || x || y), skip prefix
            uncompressed.as_bytes()[1..].to_vec()
        } else {
            // Fallback: assume raw x||y (64 bytes)
            self.public_key.clone()
        };

        let hash = Keccak256::digest(&uncompressed_xy);
        let mut addr = [0u8; 20];
        addr.copy_from_slice(&hash[12..32]);
        addr
    }
}

/// A presignature that can be consumed for one signing operation.
#[derive(Clone, ZeroizeOnDrop)]
pub struct Presignature {
    pub id: [u8; 32],
    pub data: SecureVec,
}

impl Zeroize for Presignature {
    fn zeroize(&mut self) {
        // SecureVec handles its own zeroization
        // Zero out the ID array
        self.id.zeroize();
    }
}

/// Configuration for a DKLS23 protocol session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub session_id: String,
    pub threshold: u16,
    pub total_parties: u16,
    pub party_index: PartyIndex,
}

/// A protocol message exchanged between parties during DKG/signing/resharing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolMessage {
    pub session_id: String,
    pub from: PartyIndex,
    pub to: PartyIndex,
    pub round: u16,
    pub payload: Vec<u8>,
}
