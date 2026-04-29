use k256::Scalar;
use k256::ecdsa::signature::hazmat::PrehashVerifier;
use k256::ecdsa::{Signature as K256Signature, SigningKey, VerifyingKey};
use k256::elliptic_curve::{Field, PrimeField};
use rand::rngs::OsRng;
use zeroize::Zeroize;

use super::{KeyShare, PartyIndex, SessionConfig};
use crate::errors::{MpcError, Result};

/// Simulated threshold key generation for development/testing.
///
/// In production this will delegate to the synedrion crate's CGGMP protocol.
/// For M2 milestone we use a deterministic split: generate a random signing key,
/// then Shamir-split the scalar into 3 shares with threshold 2.
///
/// SECURITY: This is NOT a real MPC DKG — the full private key exists transiently.
/// Only suitable for testnet. Production requires the real synedrion DKG where
/// the full key never materializes.
pub struct ThresholdKeyGen {
    config: SessionConfig,
}

impl ThresholdKeyGen {
    pub fn new(config: SessionConfig) -> Self {
        Self { config }
    }

    /// Run a local (non-distributed) key generation for testing.
    /// Returns key shares for all parties.
    pub fn generate_local(&self) -> Result<Vec<KeyShare>> {
        let signing_key = SigningKey::random(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        let public_key_bytes = verifying_key.to_encoded_point(false).as_bytes().to_vec();

        let secret = *signing_key.as_nonzero_scalar().as_ref();

        let shares = shamir_split(&secret, self.config.total_parties, self.config.threshold)?;

        let key_shares: Vec<KeyShare> = shares
            .into_iter()
            .enumerate()
            .map(|(i, share_bytes)| KeyShare {
                party: i as PartyIndex,
                threshold: self.config.threshold,
                total_parties: self.config.total_parties,
                secret_share: share_bytes,
                public_key: public_key_bytes.clone(),
            })
            .collect();

        Ok(key_shares)
    }
}

/// Shamir secret sharing over secp256k1 scalar field.
fn shamir_split(secret: &k256::Scalar, n: u16, t: u16) -> Result<Vec<Vec<u8>>> {
    if t < 2 || t > n {
        return Err(MpcError::DkgFailed("invalid threshold parameters".into()));
    }

    // Random polynomial coefficients: a_0 = secret, a_1..a_{t-1} = random
    let mut coeffs: Vec<Scalar> = Vec::with_capacity(t as usize);
    coeffs.push(*secret);
    for _ in 1..t {
        coeffs.push(Scalar::random(&mut OsRng));
    }

    // Evaluate polynomial at x = 1, 2, ..., n
    let mut shares = Vec::with_capacity(n as usize);
    for i in 1..=n {
        let x = Scalar::from(i as u64);
        let mut y = Scalar::ZERO;
        let mut x_pow = Scalar::ONE;
        for coeff in &coeffs {
            y += coeff * &x_pow;
            x_pow *= &x;
        }
        shares.push(y.to_bytes().to_vec());
    }

    // Zeroize coefficients
    for c in &mut coeffs {
        c.zeroize();
    }

    Ok(shares)
}

/// Reconstruct a secret from t-of-n Shamir shares using Lagrange interpolation.
pub fn shamir_reconstruct(share_indices: &[u16], share_values: &[Vec<u8>]) -> Result<Vec<u8>> {
    use k256::Scalar;
    if share_indices.len() != share_values.len() || share_indices.is_empty() {
        return Err(MpcError::SigningFailed("mismatched share data".into()));
    }

    let n = share_indices.len();
    let mut result = Scalar::ZERO;

    for i in 0..n {
        let xi = Scalar::from((share_indices[i] + 1) as u64);
        let yi_bytes: [u8; 32] = share_values[i]
            .as_slice()
            .try_into()
            .map_err(|_| MpcError::SigningFailed("invalid share length".into()))?;

        let yi = Option::<Scalar>::from(Scalar::from_repr(yi_bytes.into()))
            .ok_or_else(|| MpcError::SigningFailed("invalid scalar".into()))?;

        // Lagrange basis polynomial
        let mut lagrange = Scalar::ONE;
        for j in 0..n {
            if i == j {
                continue;
            }
            let xj = Scalar::from((share_indices[j] + 1) as u64);
            let num = Scalar::ZERO - &xj;
            let den = xi - &xj;
            lagrange *= &num * &den.invert().unwrap();
        }

        result += &yi * &lagrange;
    }

    Ok(result.to_bytes().to_vec())
}

/// Local threshold signing for testing (simulates 2-of-3).
///
/// In production this will use synedrion's online signing protocol.
/// For M2 we reconstruct the signing key from 2 shares and sign directly.
///
/// SECURITY: Same caveat as ThresholdKeyGen — full key is reconstructed.
/// Production MPC signing never reconstructs the key.
pub fn threshold_sign(
    share_indices: &[u16],
    shares: &[&KeyShare],
    message_hash: &[u8; 32],
) -> Result<(Vec<u8>, u8)> {
    if shares.len() < shares[0].threshold as usize {
        return Err(MpcError::SigningFailed(format!(
            "need {} shares, got {}",
            shares[0].threshold,
            shares.len()
        )));
    }

    let share_values: Vec<Vec<u8>> = shares.iter().map(|s| s.secret_share.clone()).collect();
    let mut secret_bytes = shamir_reconstruct(share_indices, &share_values)?;

    let secret_arr: [u8; 32] = secret_bytes
        .as_slice()
        .try_into()
        .map_err(|_| MpcError::SigningFailed("invalid secret length".into()))?;

    let signing_key = SigningKey::from_bytes(&secret_arr.into())
        .map_err(|e| MpcError::SigningFailed(e.to_string()))?;

    // Zeroize secret material
    secret_bytes.zeroize();

    let (signature, recid) = signing_key
        .sign_prehash_recoverable(message_hash)
        .map_err(|e| MpcError::SigningFailed(e.to_string()))?;

    let mut sig_bytes = Vec::with_capacity(65);
    sig_bytes.extend_from_slice(&signature.to_bytes());
    let v = recid.to_byte() + 27;
    sig_bytes.push(v);

    Ok((sig_bytes, v))
}

/// Verify an ECDSA signature against a public key using prehash.
pub fn verify_signature(
    public_key: &[u8],
    message_hash: &[u8; 32],
    signature_bytes: &[u8],
) -> Result<bool> {
    let vk = VerifyingKey::from_sec1_bytes(public_key)
        .map_err(|e| MpcError::SigningFailed(e.to_string()))?;

    let sig_data = if signature_bytes.len() == 65 {
        &signature_bytes[..64]
    } else {
        signature_bytes
    };

    let sig =
        K256Signature::from_slice(sig_data).map_err(|e| MpcError::SigningFailed(e.to_string()))?;

    Ok(vk.verify_prehash(message_hash, &sig).is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha3::Digest;

    fn test_config() -> SessionConfig {
        SessionConfig {
            session_id: "test-001".into(),
            threshold: 2,
            total_parties: 3,
            party_index: 0,
        }
    }

    #[test]
    fn test_local_keygen_produces_valid_shares() {
        let kg = ThresholdKeyGen::new(test_config());
        let shares = kg.generate_local().unwrap();

        assert_eq!(shares.len(), 3);
        // All shares have same public key
        assert_eq!(shares[0].public_key, shares[1].public_key);
        assert_eq!(shares[1].public_key, shares[2].public_key);
        // All shares are 32 bytes
        for s in &shares {
            assert_eq!(s.secret_share.len(), 32);
        }
    }

    #[test]
    fn test_shamir_roundtrip() {
        let kg = ThresholdKeyGen::new(test_config());
        let shares = kg.generate_local().unwrap();

        // Reconstruct from shares 0 and 1
        let indices = vec![0u16, 1];
        let values: Vec<Vec<u8>> = vec![
            shares[0].secret_share.clone(),
            shares[1].secret_share.clone(),
        ];
        let secret_01 = shamir_reconstruct(&indices, &values).unwrap();

        // Reconstruct from shares 0 and 2
        let indices2 = vec![0u16, 2];
        let values2: Vec<Vec<u8>> = vec![
            shares[0].secret_share.clone(),
            shares[2].secret_share.clone(),
        ];
        let secret_02 = shamir_reconstruct(&indices2, &values2).unwrap();

        // Reconstruct from shares 1 and 2
        let indices3 = vec![1u16, 2];
        let values3: Vec<Vec<u8>> = vec![
            shares[1].secret_share.clone(),
            shares[2].secret_share.clone(),
        ];
        let secret_12 = shamir_reconstruct(&indices3, &values3).unwrap();

        // All reconstructions produce the same secret
        assert_eq!(secret_01, secret_02);
        assert_eq!(secret_02, secret_12);
    }

    #[test]
    fn test_threshold_sign_and_verify() {
        let kg = ThresholdKeyGen::new(test_config());
        let shares = kg.generate_local().unwrap();

        let msg_hash = sha3::Keccak256::digest(b"hello cowallet");
        let hash: [u8; 32] = msg_hash.into();

        // Sign with parties 0 and 1
        let (sig, _v) = threshold_sign(&[0, 1], &[&shares[0], &shares[1]], &hash).unwrap();

        // Verify
        let valid = verify_signature(&shares[0].public_key, &hash, &sig).unwrap();
        assert!(valid);
    }

    #[test]
    fn test_threshold_sign_any_2_of_3() {
        let kg = ThresholdKeyGen::new(test_config());
        let shares = kg.generate_local().unwrap();

        let msg_hash = sha3::Keccak256::digest(b"test message");
        let hash: [u8; 32] = msg_hash.into();

        // All 3 combinations of 2-of-3 should produce valid signatures
        let combos: Vec<(Vec<u16>, Vec<&KeyShare>)> = vec![
            (vec![0, 1], vec![&shares[0], &shares[1]]),
            (vec![0, 2], vec![&shares[0], &shares[2]]),
            (vec![1, 2], vec![&shares[1], &shares[2]]),
        ];

        for (indices, share_refs) in &combos {
            let (sig, _v) = threshold_sign(indices, share_refs, &hash).unwrap();
            let valid = verify_signature(&shares[0].public_key, &hash, &sig).unwrap();
            assert!(valid, "failed for combo {:?}", indices);
        }
    }

    #[test]
    fn test_eth_address_from_keygen() {
        let kg = ThresholdKeyGen::new(test_config());
        let shares = kg.generate_local().unwrap();

        let addr = shares[0].eth_address();
        assert_eq!(addr.len(), 20);
        // All shares derive the same address
        assert_eq!(addr, shares[1].eth_address());
        assert_eq!(addr, shares[2].eth_address());
    }

    #[test]
    fn test_insufficient_shares_fails() {
        let kg = ThresholdKeyGen::new(test_config());
        let shares = kg.generate_local().unwrap();

        let msg_hash = [0u8; 32];
        let result = threshold_sign(&[0], &[&shares[0]], &msg_hash);
        assert!(result.is_err());
    }
}
