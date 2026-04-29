use super::DecryptedShard;
use crate::errors::{MpcError, Result};
use serde::{Deserialize, Serialize};

/// Backup shard (Shard 3) — offline cold storage held by the user.
///
/// Three backup strategies:
/// 1. Social Recovery: 3-of-5 Shamir split among trusted contacts
/// 2. Mnemonic Phrase: BIP-39-style words encoding the shard
/// 3. Hardware Key: stored on YubiKey / FIDO2 device

/// A Shamir share distributed to a trusted contact.
#[derive(Clone, Serialize, Deserialize)]
pub struct ShamirShare {
    pub index: u8,
    pub threshold: u8,
    pub total: u8,
    pub data: Vec<u8>,
}

/// Split a backup shard into Shamir shares for social recovery.
///
/// Uses Shamir's Secret Sharing over GF(256) to split the shard into
/// `total` shares, of which any `threshold` can reconstruct the original.
pub fn split_for_social_recovery(
    _shard: &DecryptedShard,
    threshold: u8,
    total: u8,
) -> Result<Vec<ShamirShare>> {
    if threshold < 2 || threshold > total {
        return Err(MpcError::ShardEncryption(format!(
            "invalid threshold: {threshold}-of-{total}"
        )));
    }

    // TODO: Implement Shamir's Secret Sharing over GF(256)
    // 1. Generate random polynomial of degree (threshold - 1) with
    //    free term = shard secret
    // 2. Evaluate at points 1..=total
    // 3. Return shares

    Err(MpcError::ShardEncryption("not yet implemented".into()))
}

/// Reconstruct a backup shard from Shamir shares.
pub fn reconstruct_from_shares(shares: &[ShamirShare]) -> Result<DecryptedShard> {
    if shares.is_empty() {
        return Err(MpcError::ShardDecryption("no shares provided".into()));
    }

    let threshold = shares[0].threshold;
    if shares.len() < threshold as usize {
        return Err(MpcError::InsufficientParties {
            required: threshold as u16,
            available: shares.len() as u16,
        });
    }

    // TODO: Implement Lagrange interpolation over GF(256)
    // 1. Use threshold shares
    // 2. Interpolate polynomial at x=0 to recover the secret

    Err(MpcError::ShardDecryption("not yet implemented".into()))
}

/// Encode a backup shard as a BIP-39-style mnemonic phrase.
pub fn encode_as_mnemonic(_shard: &DecryptedShard) -> Result<Vec<String>> {
    // TODO: Map shard bytes to BIP-39 word list
    Err(MpcError::ShardEncryption("not yet implemented".into()))
}

/// Decode a mnemonic phrase back into a backup shard.
pub fn decode_from_mnemonic(_words: &[String]) -> Result<DecryptedShard> {
    // TODO: Map BIP-39 words back to shard bytes
    Err(MpcError::ShardDecryption("not yet implemented".into()))
}
