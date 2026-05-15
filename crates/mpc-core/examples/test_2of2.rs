// Test: 2-of-2 DKG + distributed sign mimicking exact production flow
use mpc_core::dkls23::dkg::DkgSession;
use mpc_core::dkls23::sign::SignSession;
use mpc_core::dkls23::{SessionConfig, ProtocolMessage};

fn main() {
    use sha3::Digest;
    
    // === DKG (2-of-2) ===
    let config0 = SessionConfig { session_id: "dkg".into(), threshold: 2, total_parties: 2, party_index: 0 };
    let config1 = SessionConfig { session_id: "dkg".into(), threshold: 2, total_parties: 2, party_index: 1 };
    
    let mut dkg0 = DkgSession::new(config0);
    let mut dkg1 = DkgSession::new(config1);
    
    let r1_0 = dkg0.generate_round1().unwrap();
    let r1_1 = dkg1.generate_round1().unwrap();
    dkg0.process_round1(vec![ProtocolMessage { session_id: "dkg".into(), from: 1, to: 0, round: 1, payload: r1_1.payload }]).unwrap();
    dkg1.process_round1(vec![ProtocolMessage { session_id: "dkg".into(), from: 0, to: 1, round: 1, payload: r1_0.payload }]).unwrap();
    
    let r2_0 = dkg0.generate_round2().unwrap();
    let r2_1 = dkg1.generate_round2().unwrap();
    
    let msg_for_0 = r2_1.iter().find(|m| m.to == 0).unwrap();
    let msg_for_1 = r2_0.iter().find(|m| m.to == 1).unwrap();
    
    let share0 = dkg0.process_round2(vec![msg_for_0.clone()]).unwrap();
    let share1 = dkg1.process_round2(vec![msg_for_1.clone()]).unwrap();
    
    assert_eq!(share0.public_key, share1.public_key);
    println!("DKG complete: pk_len={}", share0.public_key.len());
    
    // === Simulate import_device_shard (production reload) ===
    let reimported_share0 = mpc_core::dkls23::KeyShare {
        party: 0,
        threshold: 2,
        total_parties: 2,
        secret_share: share0.secret_share.clone(),
        public_key: share0.public_key.clone(),
        paillier_pk: None,
    };
    
    // === SIGN (mimicking exact production message flow) ===
    let hash: [u8; 32] = sha3::Keccak256::digest(b"production flow test").into();
    
    // --- CLIENT: generate round 1 ---
    let client_sign_config = SessionConfig {
        session_id: "client-local-id".into(), // Different from server!
        threshold: 2,
        total_parties: 2,
        party_index: 0,
    };
    let mut client_sign = SignSession::new_distributed(client_sign_config, reimported_share0, hash);
    let client_r1 = client_sign.generate_round1().unwrap();
    
    // Client sends: [client_r1.payload] + [msg_hash] (appended)
    let mut client_payload_with_hash = client_r1.payload.clone();
    client_payload_with_hash.extend_from_slice(&hash);
    
    // --- SERVER: receives client round 1 ---
    // Server creates its own session with DIFFERENT session_id
    let server_sign_config = SessionConfig {
        session_id: "server-session-uuid".into(), // Different from client!
        threshold: 2,
        total_parties: 2,
        party_index: 1,
    };
    
    // Server extracts msg_hash from last 32 bytes
    let payload = &client_payload_with_hash;
    let msg_hash_extracted: [u8; 32] = payload[payload.len()-32..].try_into().unwrap();
    assert_eq!(msg_hash_extracted, hash);
    
    let mut server_sign = SignSession::new_distributed(server_sign_config, share1.clone(), msg_hash_extracted);
    
    // Server generates its R_1 FIRST
    let server_r1 = server_sign.generate_round1().unwrap();
    
    // Server processes client's R_0 (stripping msg_hash)
    let round1_payload = &payload[..payload.len()-32];
    let incoming = ProtocolMessage {
        session_id: "server-session-uuid".into(),
        from: 0,
        to: 1,
        round: 1,
        payload: round1_payload.to_vec(),
    };
    server_sign.process_round1(vec![incoming]).unwrap();
    
    // --- CLIENT: receives server R_1 and generates round 2 ---
    let incoming_r1 = ProtocolMessage {
        session_id: "client-local-id".into(),
        from: 1,
        to: 0,
        round: 1,
        payload: server_r1.payload,
    };
    client_sign.process_round1(vec![incoming_r1]).unwrap();
    let client_r2 = client_sign.generate_round2().unwrap();
    
    // --- SERVER: processes client round 2 (MtA) ---
    let incoming_r2 = ProtocolMessage {
        session_id: "server-session-uuid".into(),
        from: 0,
        to: 1,
        round: 2,
        payload: client_r2.payload,
    };
    let _server_result = server_sign.process_round2(vec![incoming_r2]).unwrap();
    
    // Get server's Enc(s) response
    let server_sig_payload = server_sign.get_server_response().expect("no server response");
    
    // --- CLIENT: processes server signature ---
    let incoming_sig = ProtocolMessage {
        session_id: "client-local-id".into(),
        from: 1,
        to: 0,
        round: 2,
        payload: server_sig_payload,
    };
    let sig = client_sign.process_round2(vec![incoming_sig]).unwrap();
    
    println!("Signature: v={}", sig.v);
    let verified = sig.verify(&hash, &share0.public_key).unwrap();
    println!("Verified: {}", verified);
    assert!(verified, "FAILED: production-flow 2-of-2 distributed sign");
    
    println!("\n✅ Production-flow 2-of-2 DKG + sign test PASSED!");
}
