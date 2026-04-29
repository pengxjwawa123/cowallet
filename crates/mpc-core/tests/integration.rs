use mpc_core::dkls23::{
    SessionConfig,
    dkg::DkgSession,
    presign::{PresignSession, PresignatureStore},
    sign::SignSession,
};
use sha3::Digest;

fn make_config(party: u16) -> SessionConfig {
    SessionConfig {
        session_id: "integration-test".into(),
        threshold: 2,
        total_parties: 3,
        party_index: party,
    }
}

#[test]
fn test_full_pipeline_dkg_presign_sign_verify() {
    // Phase 1: DKG — generate key shares for 3 parties
    let mut dkg = DkgSession::new(make_config(0));
    let shares = dkg.run_local().unwrap();
    assert_eq!(shares.len(), 3);

    let public_key = shares[0].public_key.clone();
    for s in &shares {
        assert_eq!(s.public_key, public_key);
    }

    // Phase 2: Presign — parties 0 and 1 create a presignature
    let mut presign = PresignSession::new_local(
        make_config(0),
        vec![0, 1],
        vec![shares[0].clone(), shares[1].clone()],
    );
    let presig = presign.run_local().unwrap();

    let mut store = PresignatureStore::new();
    store.add(presig);
    assert_eq!(store.count(), 1);

    // Phase 3: Sign — consume presignature, sign a message
    let msg_hash: [u8; 32] = sha3::Keccak256::digest(b"send 0.1 ETH to alice.eth").into();

    let _presig = store.take().unwrap();
    assert!(store.is_empty());

    let mut sign_session = SignSession::new_local(
        make_config(0),
        vec![0, 1],
        vec![shares[0].clone(), shares[1].clone()],
        msg_hash,
    );
    let sig = sign_session.sign_local().unwrap();

    // Phase 4: Verify
    assert!(sig.verify(&msg_hash, &public_key).unwrap());

    // Verify signature encoding roundtrip
    let bytes = sig.to_bytes();
    assert_eq!(bytes.len(), 65);
    assert!(bytes[64] == 27 || bytes[64] == 28);
}

#[test]
fn test_all_party_combinations_produce_valid_signatures() {
    let mut dkg = DkgSession::new(make_config(0));
    let shares = dkg.run_local().unwrap();
    let public_key = shares[0].public_key.clone();

    let combos: [(Vec<u16>, usize, usize); 3] =
        [(vec![0, 1], 0, 1), (vec![0, 2], 0, 2), (vec![1, 2], 1, 2)];

    for (indices, a, b) in &combos {
        let msg_hash: [u8; 32] =
            sha3::Keccak256::digest(format!("msg for combo {a}-{b}").as_bytes()).into();

        let mut sign_session = SignSession::new_local(
            make_config(0),
            indices.clone(),
            vec![shares[*a].clone(), shares[*b].clone()],
            msg_hash,
        );
        let sig = sign_session.sign_local().unwrap();
        assert!(
            sig.verify(&msg_hash, &public_key).unwrap(),
            "verification failed for parties {a} and {b}"
        );
    }
}

#[test]
fn test_eth_address_consistent_across_shares() {
    let mut dkg = DkgSession::new(make_config(0));
    let shares = dkg.run_local().unwrap();

    let addr = shares[0].eth_address();
    assert_eq!(addr.len(), 20);
    assert_eq!(addr, shares[1].eth_address());
    assert_eq!(addr, shares[2].eth_address());
}

#[test]
fn test_different_messages_produce_different_signatures() {
    let mut dkg = DkgSession::new(make_config(0));
    let shares = dkg.run_local().unwrap();

    let hash_a: [u8; 32] = sha3::Keccak256::digest(b"message A").into();
    let hash_b: [u8; 32] = sha3::Keccak256::digest(b"message B").into();

    let mut session_a = SignSession::new_local(
        make_config(0),
        vec![0, 1],
        vec![shares[0].clone(), shares[1].clone()],
        hash_a,
    );
    let sig_a = session_a.sign_local().unwrap();

    let mut session_b = SignSession::new_local(
        make_config(0),
        vec![0, 1],
        vec![shares[0].clone(), shares[1].clone()],
        hash_b,
    );
    let sig_b = session_b.sign_local().unwrap();

    assert_ne!(sig_a.to_bytes(), sig_b.to_bytes());

    // But both verify against their respective hashes
    assert!(sig_a.verify(&hash_a, &shares[0].public_key).unwrap());
    assert!(sig_b.verify(&hash_b, &shares[0].public_key).unwrap());

    // Cross-verify should fail
    assert!(!sig_a.verify(&hash_b, &shares[0].public_key).unwrap());
}

#[test]
fn test_presignature_pool_multiple() {
    let mut dkg = DkgSession::new(make_config(0));
    let shares = dkg.run_local().unwrap();

    let mut store = PresignatureStore::new();

    // Generate 5 presignatures from different party combinations
    for i in 0..5 {
        let (indices, a, b) = if i % 3 == 0 {
            (vec![0u16, 1], 0, 1)
        } else if i % 3 == 1 {
            (vec![0, 2], 0, 2)
        } else {
            (vec![1, 2], 1, 2)
        };

        let mut presign = PresignSession::new_local(
            make_config(0),
            indices,
            vec![shares[a].clone(), shares[b].clone()],
        );
        store.add(presign.run_local().unwrap());
    }

    assert_eq!(store.count(), 5);

    // All presignatures have unique IDs
    let mut ids = Vec::new();
    while let Some(p) = store.take() {
        assert!(!ids.contains(&p.id));
        ids.push(p.id);
    }
    assert_eq!(ids.len(), 5);
}
