use mpc_core::dkls23::{
    SessionConfig,
    dkg::DkgSession,
    presign::{PresignSession, PresignatureStore, decode_presign_data},
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
    let mut dkg = DkgSession::new(make_config(0));
    let shares = dkg.run_local().unwrap();
    assert_eq!(shares.len(), 3);

    let public_key = shares[0].public_key.clone();
    for s in &shares {
        assert_eq!(s.public_key, public_key);
    }

    // Presign between party 0 and party 1
    let mut ps0 = PresignSession::new(make_config(0));
    let mut ps1 = PresignSession::new(make_config(1));

    let msgs0 = ps0.generate_round1().unwrap();
    let msgs1 = ps1.generate_round1().unwrap();
    ps0.process_round1(msgs1).unwrap();
    ps1.process_round1(msgs0).unwrap();

    let presig0 = ps0.finalize().unwrap();
    let presig1 = ps1.finalize().unwrap();

    let mut store = PresignatureStore::new();
    store.add(presig0.clone());
    assert_eq!(store.count(), 1);

    // Sign using presign data
    let msg_hash: [u8; 32] = sha3::Keccak256::digest(b"send 0.1 ETH to alice.eth").into();

    let _taken = store.take().unwrap();
    assert!(store.is_empty());

    let data0 = decode_presign_data(&presig0).unwrap();
    let data1 = decode_presign_data(&presig1).unwrap();

    let mut sign0 = SignSession::new_distributed(make_config(0), shares[0].clone(), msg_hash);
    let mut sign1 = SignSession::new_distributed(make_config(1), shares[1].clone(), msg_hash);

    let r1_msg0 = sign0.generate_round1_with_presign(&data0.k_i, &data0.r_i).unwrap();
    let r1_msg1 = sign1.generate_round1_with_presign(&data1.k_i, &data1.r_i).unwrap();

    sign0.process_round1(vec![r1_msg1]).unwrap();
    sign1.process_round1(vec![r1_msg0]).unwrap();

    let r2_msg0 = sign0.generate_round2().unwrap();
    let _sig_server = sign1.process_round2(vec![r2_msg0]).unwrap();

    let payload = sign1.get_server_response()
        .expect("server should produce ServerSignature");
    let server_response = mpc_core::dkls23::ProtocolMessage {
        session_id: "integration-test".into(),
        from: 1,
        to: 0,
        round: 2,
        payload,
    };

    let sig = sign0.process_round2(vec![server_response]).unwrap();
    assert!(sig.verify(&msg_hash, &public_key).unwrap());

    let bytes = sig.to_bytes();
    assert_eq!(bytes.len(), 65);
    assert!(bytes[64] == 27 || bytes[64] == 28);
}

#[test]
fn test_all_party_combinations_produce_valid_signatures() {
    let mut dkg = DkgSession::new(make_config(0));
    let shares = dkg.run_local().unwrap();
    let public_key = shares[0].public_key.clone();

    let combos: [(u16, u16); 3] = [(0, 1), (0, 2), (1, 2)];

    for (a, b) in &combos {
        let msg_hash: [u8; 32] =
            sha3::Keccak256::digest(format!("msg for combo {a}-{b}").as_bytes()).into();

        let mut sign_session = SignSession::new_local(
            make_config(0),
            vec![*a, *b],
            vec![shares[*a as usize].clone(), shares[*b as usize].clone()],
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

    assert!(sig_a.verify(&hash_a, &shares[0].public_key).unwrap());
    assert!(sig_b.verify(&hash_b, &shares[0].public_key).unwrap());

    // Cross-verify should fail
    assert!(!sig_a.verify(&hash_b, &shares[0].public_key).unwrap());
}

#[test]
fn test_presignature_pool_multiple() {
    let mut store = PresignatureStore::new();

    for _ in 0..5 {
        let mut ps0 = PresignSession::new(make_config(0));
        let mut ps1 = PresignSession::new(make_config(1));

        let msgs0 = ps0.generate_round1().unwrap();
        let msgs1 = ps1.generate_round1().unwrap();
        ps0.process_round1(msgs1).unwrap();
        ps1.process_round1(msgs0).unwrap();

        store.add(ps0.finalize().unwrap());
    }

    assert_eq!(store.count(), 5);

    let mut ids = Vec::new();
    while let Some(p) = store.take() {
        assert!(!ids.contains(&p.id));
        ids.push(p.id);
    }
    assert_eq!(ids.len(), 5);
}
