use alloy_primitives::{Address, B256};
use alloy_signer::Signature;
use mpc_core::dkls23::{KeyShare, SessionConfig, sign::SignSession};

/// MPC-backed signer implementing the alloy Signer trait.
///
/// For M2/testnet: holds key shares directly and signs locally.
/// Production will decrypt device shard + coordinate with server.
pub struct MpcSigner {
    pub address: Address,
    pub chain_id: u64,
    share_indices: Vec<u16>,
    shares: Vec<KeyShare>,
}

impl MpcSigner {
    /// Create a local-mode signer from pre-existing key shares.
    /// M2/testnet only — production will use encrypted shards + server.
    pub fn from_shares(
        address: Address,
        chain_id: u64,
        share_indices: Vec<u16>,
        shares: Vec<KeyShare>,
    ) -> Self {
        Self {
            address,
            chain_id,
            share_indices,
            shares,
        }
    }

    pub(crate) fn sign_hash_inner(&self, hash: &B256) -> Result<Signature, MpcSignerError> {
        let msg_hash: [u8; 32] = hash.0;

        let config = SessionConfig {
            session_id: format!("sign-{}", uuid::Uuid::new_v4()),
            threshold: self.shares[0].threshold,
            total_parties: self.shares[0].total_parties,
            party_index: self.share_indices[0],
        };

        let mut session = SignSession::new_local(
            config,
            self.share_indices.clone(),
            self.shares.clone(),
            msg_hash,
        );

        let ecdsa_sig = session
            .sign_local()
            .map_err(|e| MpcSignerError::ProtocolError(e.to_string()))?;

        let r = B256::from(ecdsa_sig.r);
        let s = B256::from(ecdsa_sig.s);

        // Try all 4 possible recovery IDs to find the one that recovers to our address
        for recovery_id in 0..4u8 {
            let y_parity = (recovery_id & 1) == 1;
            let sig = Signature::from_scalars_and_parity(r, s, y_parity);
            
            if let Ok(recovered_addr) = sig.recover_address_from_prehash(hash) {
                if recovered_addr == self.address {
                    return Ok(sig);
                }
            }
        }

        // If none of the recovery IDs worked, return an error
        Err(MpcSignerError::ProtocolError(
            "failed to find valid recovery ID for signature".to_string(),
        ))
    }
}

#[async_trait::async_trait]
impl alloy_signer::Signer for MpcSigner {
    async fn sign_hash(&self, hash: &B256) -> Result<Signature, alloy_signer::Error> {
        self.sign_hash_inner(hash)
            .map_err(|e| alloy_signer::Error::Other(Box::new(e)))
    }

    fn address(&self) -> Address {
        self.address
    }

    fn chain_id(&self) -> Option<u64> {
        Some(self.chain_id)
    }

    fn set_chain_id(&mut self, chain_id: Option<u64>) {
        if let Some(id) = chain_id {
            self.chain_id = id;
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MpcSignerError {
    #[error("biometric authentication failed")]
    BiometricFailed,

    #[error("server unreachable: {0}")]
    ServerError(String),

    #[error("signing protocol failed: {0}")]
    ProtocolError(String),

    #[error("timeout waiting for server")]
    Timeout,
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_signer::Signer;
    use mpc_core::dkls23::dkg::DkgSession;
    use sha3::Digest;

    fn setup_signer() -> (MpcSigner, Vec<KeyShare>) {
        let config = SessionConfig {
            session_id: "test".into(),
            threshold: 2,
            total_parties: 3,
            party_index: 0,
        };
        let mut dkg = DkgSession::new(config);
        let shares = dkg.run_local().unwrap();

        let eth_addr = shares[0].eth_address();
        let address = Address::from_slice(&eth_addr);

        let signer = MpcSigner::from_shares(
            address,
            84532, // Base Sepolia
            vec![0, 1],
            vec![shares[0].clone(), shares[1].clone()],
        );
        (signer, shares)
    }

    #[tokio::test]
    async fn test_mpc_signer_sign_hash() {
        let (signer, _shares) = setup_signer();

        let digest: [u8; 32] = sha3::Keccak256::digest(b"test tx").into();
        let hash = B256::from(digest);
        let sig = signer.sign_hash(&hash).await.unwrap();

        // Recover address from signature
        let recovered = sig.recover_address_from_prehash(&hash).unwrap();
        assert_eq!(recovered, signer.address());
    }

    #[tokio::test]
    async fn test_mpc_signer_chain_id() {
        let (mut signer, _) = setup_signer();

        assert_eq!(signer.chain_id(), Some(84532));
        signer.set_chain_id(Some(1));
        assert_eq!(signer.chain_id(), Some(1));
    }

    #[tokio::test]
    async fn test_mpc_signer_different_messages() {
        let (signer, _) = setup_signer();

        let digest_a: [u8; 32] = sha3::Keccak256::digest(b"msg A").into();
        let hash_a = B256::from(digest_a);
        let digest_b: [u8; 32] = sha3::Keccak256::digest(b"msg B").into();
        let hash_b = B256::from(digest_b);

        let sig_a = signer.sign_hash(&hash_a).await.unwrap();
        let sig_b = signer.sign_hash(&hash_b).await.unwrap();

        assert_ne!(sig_a.as_bytes(), sig_b.as_bytes());

        // Both recover to the same address
        let addr_a = sig_a.recover_address_from_prehash(&hash_a).unwrap();
        let addr_b = sig_b.recover_address_from_prehash(&hash_b).unwrap();
        assert_eq!(addr_a, addr_b);
        assert_eq!(addr_a, signer.address());
    }
}
