/// FFI-safe API surface for flutter_rust_bridge.
///
/// Design rules:
/// 1. Only primitive types, String, Vec<u8>, and simple structs cross the boundary
/// 2. Secret material stays in Rust (state.rs) — Dart gets addresses and public keys
/// 3. All async Rust work uses the shared tokio runtime
/// 4. Errors are returned as Result<T, String> for simple FFI mapping
use alloy_primitives::{Address, U256};
use mpc_core::dkls23::dkg::DkgSession;
use mpc_core::dkls23::protocol::ThresholdKeyGen;
use mpc_core::dkls23::reshare::ReshareSession;
use mpc_core::dkls23::{ProtocolMessage, SessionConfig};
use serde_json;

use crate::state;

// ---------------------------------------------------------------------------
// Types returned to Dart
// ---------------------------------------------------------------------------

pub struct FfiWalletInfo {
    pub address: String,
    pub public_key: Vec<u8>,
}

pub struct FfiBalance {
    pub wei: String,
    pub formatted: String,
    pub decimals: u8,
}

pub struct FfiTxResult {
    pub tx_hash: String,
}

pub struct FfiGasEstimate {
    pub gas_limit: u64,
    pub max_fee_per_gas: String,
    pub max_priority_fee_per_gas: String,
    pub estimated_cost_wei: String,
}

pub struct FfiKeyStatus {
    pub has_device_shard: bool,
    pub has_server_shard: bool,
    pub has_backup_shard: bool,
    pub address: String,
}

// ---------------------------------------------------------------------------
// DKG Protocol types (FFI-safe serialization)
// ---------------------------------------------------------------------------

pub struct FfiDkgSession {
    /// Session ID for message routing
    pub session_id: String,
}

pub struct FfiRound1Result {
    /// Serialized ProtocolMessage (JSON)
    pub message_json: String,
}

pub struct FfiRound2Result {
    /// Serialized ProtocolMessage (JSON)
    pub message_json: String,
}

pub struct FfiDkgComplete {
    /// Wallet address (0x-prefixed hex)
    pub address: String,
    /// Public key (compressed SEC1 format)
    pub public_key: Vec<u8>,
}

// ---------------------------------------------------------------------------
// Wallet operations
// ---------------------------------------------------------------------------

/// Generate a new MPC wallet using local-mode TSS (2-of-3).
/// Returns the derived Ethereum address and public key.
/// The 3 key shares are stored in Rust memory — NEVER sent to Dart.
pub fn generate_wallet() -> Result<FfiWalletInfo, String> {
    let config = SessionConfig {
        session_id: uuid::Uuid::new_v4().to_string(),
        threshold: 2,
        total_parties: 3,
        party_index: 0,
    };

    let keygen = ThresholdKeyGen::new(config);
    let shares = keygen.generate_local().map_err(|e| e.to_string())?;

    let address = shares[0].eth_address();
    let address_hex = format!(
        "0x{}",
        address.iter().map(|b| format!("{b:02x}")).collect::<String>()
    );
    let public_key = shares[0].public_key.clone();

    state::store_shares(shares);

    Ok(FfiWalletInfo {
        address: address_hex,
        public_key,
    })
}

/// Check if wallet shares are loaded in memory.
pub fn has_wallet() -> bool {
    state::has_shares()
}

/// Get the key status for the UI (which shards are present).
pub fn get_key_status() -> FfiKeyStatus {
    FfiKeyStatus {
        has_device_shard: state::get_share(0).is_some(),
        has_server_shard: state::get_share(1).is_some(),
        has_backup_shard: state::get_share(2).is_some(),
        address: state::get_share(0)
            .map(|s| {
                let addr = s.eth_address();
                format!(
                    "0x{}",
                    addr.iter().map(|b| format!("{b:02x}")).collect::<String>()
                )
            })
            .unwrap_or_default(),
    }
}

/// Clear all shares from memory (for wallet deletion / reset).
pub fn clear_wallet() {
    state::clear_shares();
}

// ---------------------------------------------------------------------------
// DKG Protocol — Distributed Key Generation (3-round protocol)
// ---------------------------------------------------------------------------

/// Initialize a new DKG session.
/// Returns the session_id to use for subsequent calls.
pub fn dkg_session_new(party_index: u16) -> Result<FfiDkgSession, String> {
    let session_id = uuid::Uuid::new_v4().to_string();
    
    let config = SessionConfig {
        session_id: session_id.clone(),
        threshold: 2,
        total_parties: 2,
        party_index,
    };
    
    let dkg = DkgSession::new(config);
    state::create_dkg_session(session_id.clone(), dkg);
    
    Ok(FfiDkgSession { session_id })
}

/// DKG Round 1: Generate commitments to VSS polynomial.
/// Returns the broadcast message as JSON.
pub fn dkg_generate_round1(session_id: String) -> Result<FfiRound1Result, String> {
    let arc_dkg = state::get_dkg_session_arc(&session_id)
        .ok_or("DKG session not found")?;
    
    let msg = {
        let mut dkg = arc_dkg.lock().unwrap();
        dkg.generate_round1()
            .map_err(|e| format!("Round 1 generation failed: {}", e))?
    };
    
    // Serialize message to JSON for Dart
    let message_json = serde_json::to_string(&msg)
        .map_err(|e| format!("Serialization failed: {}", e))?;
    
    Ok(FfiRound1Result { message_json })
}

/// DKG Round 1: Process all commitments from other parties.
pub fn dkg_process_round1(
    session_id: String,
    messages_json: Vec<String>,
) -> Result<(), String> {
    let arc_dkg = state::get_dkg_session_arc(&session_id)
        .ok_or("DKG session not found")?;
    
    let mut messages = Vec::new();
    for msg_json in messages_json {
        let msg: ProtocolMessage = serde_json::from_str(&msg_json)
            .map_err(|e| format!("Failed to deserialize message: {}", e))?;
        messages.push(msg);
    }
    
    let mut dkg = arc_dkg.lock().unwrap();
    dkg.process_round1(messages)
        .map_err(|e| format!("Round 1 processing failed: {}", e))?;
    
    Ok(())
}

/// DKG Round 2: Generate secret share evaluations.
/// Returns the message(s) to send to other parties as JSON.
pub fn dkg_generate_round2(session_id: String) -> Result<Vec<String>, String> {
    let arc_dkg = state::get_dkg_session_arc(&session_id)
        .ok_or("DKG session not found")?;
    
    let messages = {
        let mut dkg = arc_dkg.lock().unwrap();
        dkg.generate_round2()
            .map_err(|e| format!("Round 2 generation failed: {}", e))?
    };
    
    let json_messages = messages
        .into_iter()
        .map(|msg| serde_json::to_string(&msg)
            .map_err(|e| format!("Serialization failed: {}", e)))
        .collect::<Result<Vec<_>, _>>()?;
    
    Ok(json_messages)
}

/// DKG Round 2: Process share evaluations from other parties.
pub fn dkg_process_round2(
    session_id: String,
    messages_json: Vec<String>,
) -> Result<(), String> {
    let arc_dkg = state::get_dkg_session_arc(&session_id)
        .ok_or("DKG session not found")?;
    
    let mut messages = Vec::new();
    for msg_json in messages_json {
        let msg: ProtocolMessage = serde_json::from_str(&msg_json)
            .map_err(|e| format!("Failed to deserialize message: {}", e))?;
        messages.push(msg);
    }
    
    let mut dkg = arc_dkg.lock().unwrap();
    let _key_share = dkg.process_round2(messages)
        .map_err(|e| format!("Round 2 processing failed: {}", e))?;
    
    Ok(())
}

/// Finalize DKG and extract the key share.
/// Stores the key shares in memory for later signing.
/// Does NOT delete the DKG session — call dkg_derive_backup_share first if needed.
pub fn dkg_finalize(session_id: String) -> Result<FfiDkgComplete, String> {
    let arc_dkg = state::get_dkg_session_arc(&session_id)
        .ok_or("DKG session not found")?;

    let key_share = {
        let dkg = arc_dkg.lock().unwrap();
        dkg.finalize()
            .map_err(|e| format!("DKG finalization failed: {}", e))?
    };

    let addr = key_share.eth_address();
    let address_hex = format!(
        "0x{}",
        addr.iter().map(|b| format!("{b:02x}")).collect::<String>()
    );
    let public_key = key_share.public_key.clone();

    state::store_shares(vec![key_share]);

    Ok(FfiDkgComplete {
        address: address_hex,
        public_key,
    })
}

/// Derive the backup shard (Party 2) from DKG round 2 evaluations.
/// Must be called AFTER dkg_finalize but BEFORE the session is cleaned up.
/// Returns the raw 32-byte secret share for the backup party.
pub fn dkg_derive_backup_share(session_id: String, backup_party_index: u16) -> Result<Vec<u8>, String> {
    let arc_dkg = state::get_dkg_session_arc(&session_id)
        .ok_or("DKG session not found")?;

    let backup_share = {
        let dkg = arc_dkg.lock().unwrap();
        dkg.derive_backup_share(backup_party_index)
            .map_err(|e| format!("Failed to derive backup share: {}", e))?
    };

    // Clean up now that we've extracted everything
    state::delete_dkg_session(&session_id);

    Ok(backup_share.secret_share.as_bytes().to_vec())
}

/// Combine two backup share contributions into the final backup shard.
/// Adds device_share + server_share modulo the secp256k1 curve order.
/// Both inputs must be exactly 32 bytes.
pub fn combine_backup_shares(device_share: Vec<u8>, server_share: Vec<u8>) -> Result<Vec<u8>, String> {
    use k256::elliptic_curve::PrimeField;
    use k256::Scalar;

    if device_share.len() != 32 {
        return Err(format!("device_share must be 32 bytes, got {}", device_share.len()));
    }
    if server_share.len() != 32 {
        return Err(format!("server_share must be 32 bytes, got {}", server_share.len()));
    }

    // Parse device contribution
    let mut device_bytes = [0u8; 32];
    device_bytes.copy_from_slice(&device_share);
    let device_scalar = Option::<Scalar>::from(Scalar::from_repr(device_bytes.into()))
        .ok_or_else(|| "invalid device_share: not a valid secp256k1 scalar".to_string())?;

    // Parse server contribution
    let mut server_bytes = [0u8; 32];
    server_bytes.copy_from_slice(&server_share);
    let server_scalar = Option::<Scalar>::from(Scalar::from_repr(server_bytes.into()))
        .ok_or_else(|| "invalid server_share: not a valid secp256k1 scalar".to_string())?;

    // Add modulo curve order: backup_share = device_share + server_share (mod n)
    let combined_scalar = device_scalar + server_scalar;

    Ok(combined_scalar.to_bytes().to_vec())
}

// ---------------------------------------------------------------------------
// Distributed Signing — 2-party ECDSA without key reconstruction
// ---------------------------------------------------------------------------

pub struct FfiSignRound1 {
    /// Session ID for routing subsequent rounds
    pub session_id: String,
    /// Serialized Round 1 payload (bincode of SignRound1Message) to send to server
    pub payload: Vec<u8>,
    /// The message hash included for the server
    pub msg_hash: Vec<u8>,
}

pub struct FfiSignResult {
    /// 65-byte signature (r[32] || s[32] || v[1])
    pub signature: Vec<u8>,
}

/// Initialize a distributed signing session and generate Round 1 (R_0 = k_0*G).
/// Returns the payload to send to the server.
pub fn sign_generate_round1(msg_hash: Vec<u8>) -> Result<FfiSignRound1, String> {
    if msg_hash.len() != 32 {
        return Err("msg_hash must be 32 bytes".into());
    }

    let share0 = state::get_share(0).ok_or("device shard not loaded")?;

    let msg_arr: [u8; 32] = msg_hash.clone().try_into().map_err(|_| "msg_hash must be 32 bytes")?;

    let config = SessionConfig {
        session_id: uuid::Uuid::new_v4().to_string(),
        threshold: 2,
        total_parties: share0.total_parties,
        party_index: share0.party,
    };

    let mut session = mpc_core::dkls23::sign::SignSession::new_distributed(
        config,
        share0,
        msg_arr,
    );

    let round1_msg = session.generate_round1()
        .map_err(|e| format!("sign round1 generation failed: {}", e))?;

    let session_id = round1_msg.session_id.clone();
    state::create_sign_session(session_id, session);

    Ok(FfiSignRound1 {
        session_id: round1_msg.session_id.clone(),
        payload: round1_msg.payload,
        msg_hash,
    })
}

/// Process server's Round 1 response (R_1) and generate device's Round 2 contribution.
/// Returns the DeviceContribution payload to send to server.
pub fn sign_process_round1_and_generate_round2(
    session_id: String,
    server_round1_payload: Vec<u8>,
) -> Result<Vec<u8>, String> {
    let arc_session = state::get_sign_session_arc(&session_id)
        .ok_or("sign session not found")?;

    let mut session = arc_session.lock().unwrap();

    // Process server's R_1
    let incoming = ProtocolMessage {
        session_id: session_id.clone(),
        from: 1, // server
        to: 0,   // device
        round: 1,
        payload: server_round1_payload,
    };
    session.process_round1(vec![incoming])
        .map_err(|e| format!("process server R1 failed: {}", e))?;

    // Generate device's Round 2 (DeviceContribution: c_0, k_0_inv)
    let round2_msg = session.generate_round2()
        .map_err(|e| format!("generate round2 failed: {}", e))?;

    Ok(round2_msg.payload)
}

/// Process server's Round 2 response (ServerSignature containing s).
/// Returns the final 65-byte signature.
pub fn sign_process_round2(
    session_id: String,
    server_round2_payload: Vec<u8>,
) -> Result<FfiSignResult, String> {
    let arc_session = state::get_sign_session_arc(&session_id)
        .ok_or("sign session not found")?;

    let mut session = arc_session.lock().unwrap();

    let incoming = ProtocolMessage {
        session_id: session_id.clone(),
        from: 1,
        to: 0,
        round: 2,
        payload: server_round2_payload,
    };

    let sig = session.process_round2(vec![incoming])
        .map_err(|e| format!("process server signature failed: {}", e))?;

    let sig_bytes = sig.to_bytes();

    // Clean up session
    drop(session);
    state::delete_sign_session(&session_id);

    Ok(FfiSignResult {
        signature: sig_bytes.to_vec(),
    })
}

// ---------------------------------------------------------------------------
// Reshare Protocol — Proactive key refresh (preserves public key)
// ---------------------------------------------------------------------------

pub struct FfiReshareSession {
    pub session_id: String,
}

pub struct FfiReshareRound1Result {
    /// JSON-serialized list of ProtocolMessages (one per party)
    pub messages_json: Vec<String>,
}

pub struct FfiReshareComplete {
    pub address: String,
    pub public_key: Vec<u8>,
}

/// Initialize a reshare session using the current device shard.
/// The old shard is consumed; after finalize() the new shard replaces it.
pub fn reshare_session_new(party_index: u16) -> Result<FfiReshareSession, String> {
    let old_share = state::get_share(party_index)
        .ok_or("device shard not loaded — cannot reshare")?;

    let session_id = uuid::Uuid::new_v4().to_string();

    let config = SessionConfig {
        session_id: session_id.clone(),
        threshold: old_share.threshold,
        total_parties: old_share.total_parties,
        party_index,
    };

    let reshare = ReshareSession::new(config, old_share);
    state::create_reshare_session(session_id.clone(), reshare);

    Ok(FfiReshareSession { session_id })
}

/// Generate reshare Round 1 messages (new VSS polynomial evaluations for each party).
/// Returns serialized ProtocolMessages to send to other parties.
pub fn reshare_generate_round1(session_id: String) -> Result<FfiReshareRound1Result, String> {
    let arc = state::get_reshare_session_arc(&session_id)
        .ok_or("reshare session not found")?;

    let messages = {
        let mut session = arc.lock().unwrap();
        session.generate_round1()
            .map_err(|e| format!("reshare round1 failed: {}", e))?
    };

    let json_messages = messages
        .into_iter()
        .map(|msg| serde_json::to_string(&msg)
            .map_err(|e| format!("serialization failed: {}", e)))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(FfiReshareRound1Result { messages_json: json_messages })
}

/// Process reshare Round 1 messages from other parties and compute new key share.
pub fn reshare_process_round1(
    session_id: String,
    messages_json: Vec<String>,
) -> Result<(), String> {
    let arc = state::get_reshare_session_arc(&session_id)
        .ok_or("reshare session not found")?;

    let mut messages = Vec::new();
    for msg_json in messages_json {
        let msg: ProtocolMessage = serde_json::from_str(&msg_json)
            .map_err(|e| format!("failed to deserialize reshare message: {}", e))?;
        messages.push(msg);
    }

    let mut session = arc.lock().unwrap();
    session.process_round1(messages)
        .map_err(|e| format!("reshare process_round1 failed: {}", e))?;

    Ok(())
}

/// Finalize reshare: extract the new key share and replace the old one in memory.
/// After this call, the old share is invalid and the new share is active.
pub fn reshare_finalize(session_id: String) -> Result<FfiReshareComplete, String> {
    let arc = state::get_reshare_session_arc(&session_id)
        .ok_or("reshare session not found")?;

    let new_share = {
        let mut session = arc.lock().unwrap();
        session.finalize()
            .map_err(|e| format!("reshare finalization failed: {}", e))?
    };

    let addr = new_share.eth_address();
    let address_hex = format!(
        "0x{}",
        addr.iter().map(|b| format!("{b:02x}")).collect::<String>()
    );
    let public_key = new_share.public_key.clone();

    // Replace old share with the new one
    state::store_shares(vec![new_share]);

    // Clean up session
    state::delete_reshare_session(&session_id);

    Ok(FfiReshareComplete {
        address: address_hex,
        public_key,
    })
}

// ---------------------------------------------------------------------------
// Presign Protocol — Pre-compute signing material for instant signatures
// ---------------------------------------------------------------------------

pub struct FfiPresignRound1 {
    pub session_id: String,
    pub payload: Vec<u8>,
}

pub struct FfiPresignComplete {
    /// Serialized presignature data (bincode-encoded PresignData)
    pub presig_data: Vec<u8>,
}

/// Start a presign session: generate ephemeral k_i and Round 1 message (R_i = k_i*G).
pub fn presign_generate_round1() -> Result<FfiPresignRound1, String> {
    use mpc_core::dkls23::presign::PresignSession;

    let share0 = state::get_share(0).ok_or("device shard not loaded")?;

    let session_id = uuid::Uuid::new_v4().to_string();
    let config = SessionConfig {
        session_id: session_id.clone(),
        threshold: 2,
        total_parties: share0.total_parties,
        party_index: share0.party,
    };

    let mut session = PresignSession::new(config);

    let round1_msgs = session.generate_round1()
        .map_err(|e| format!("presign round1 generation failed: {}", e))?;

    let payload = round1_msgs.first()
        .ok_or("no round1 message generated")?
        .payload.clone();

    state::create_presign_session(session_id.clone(), session);

    Ok(FfiPresignRound1 {
        session_id,
        payload,
    })
}

/// Process server's presign Round 1 message and compute aggregate R.
/// Returns an empty Vec on success (protocol complete, call presign_finalize to extract data).
pub fn presign_process_round1_and_generate_round2(
    session_id: String,
    server_round1_payload: Vec<u8>,
) -> Result<Vec<u8>, String> {
    use mpc_core::dkls23::ProtocolMessage;

    let arc_session = state::get_presign_session_arc(&session_id)
        .ok_or("presign session not found")?;

    let mut session = arc_session.lock().unwrap();

    let server_msg = ProtocolMessage {
        session_id: session_id.clone(),
        from: 1, // server party
        to: 0,
        round: 1,
        payload: server_round1_payload,
    };

    session.process_round1(vec![server_msg])
        .map_err(|e| format!("presign process_round1 failed: {}", e))?;

    Ok(Vec::new())
}

/// Finalize presign: extract the presignature data for storage.
pub fn presign_finalize(session_id: String) -> Result<FfiPresignComplete, String> {
    let arc_session = state::get_presign_session_arc(&session_id)
        .ok_or("presign session not found")?;

    let session = arc_session.lock().unwrap();

    let presig = session.finalize()
        .map_err(|e| format!("presign finalize failed: {}", e))?;

    drop(session);
    state::delete_presign_session(&session_id);

    Ok(FfiPresignComplete {
        presig_data: presig.data.as_bytes().to_vec(),
    })
}

// ---------------------------------------------------------------------------
// Recovery Protocol — Restore device shard using backup + server shards
// ---------------------------------------------------------------------------

/// Import and validate the backup shard for recovery.
/// The backup shard is stored temporarily until recovery is complete.
pub fn recovery_import_backup_shard(backup_bytes: Vec<u8>) -> Result<(), String> {
    // Validate backup bytes length (should be 32 bytes for the secret share)
    if backup_bytes.len() != 32 {
        return Err(format!("invalid backup shard length: expected 32 bytes, got {}", backup_bytes.len()));
    }

    // Parse and validate the secret share
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&backup_bytes);

    use k256::elliptic_curve::PrimeField;
    use k256::Scalar;

    let _secret = Option::<Scalar>::from(Scalar::from_repr(bytes.into()))
        .ok_or_else(|| "invalid backup shard: not a valid scalar".to_string())?;

    // Store as a temporary KeyShare (Party 2)
    // Note: We don't have the full public key yet, but we'll get it from the server
    let backup_share = mpc_core::dkls23::KeyShare {
        party: 2,
        threshold: 2,
        total_parties: 3,
        secret_share: backup_bytes.into(),
        public_key: Vec::new(), // Will be populated during recovery
        paillier_pk: None, // Backup shard doesn't participate in signing
    };

    state::store_recovery_backup_shard(backup_share);

    Ok(())
}

/// Reconstruct the device shard (Party 0) using backup shard + server contribution.
///
/// Recovery flow:
/// 1. User authenticates and server fetches its shard (Party 1)
/// 2. User provides backup shard (Party 2)
/// 3. Server initiates a special reshare where Party 0 is reconstructed
/// 4. The backup + server shards generate a new device shard without changing the public key
pub fn recovery_reconstruct_device_shard(
    session_id: String,
    server_messages_json: Vec<String>,
    public_key: Vec<u8>,
) -> Result<FfiDkgComplete, String> {
    let backup_share = state::get_recovery_backup_shard()
        .ok_or("backup shard not imported — call recovery_import_backup_shard first")?;

    // Update backup share with the correct public key from server
    let mut backup_share = backup_share;
    backup_share.public_key = public_key.clone();

    // Create a special reshare session using the backup shard
    // This simulates Party 2 participating in a reshare to generate Party 0
    let config = SessionConfig {
        session_id: session_id.clone(),
        threshold: 2,
        total_parties: 3,
        party_index: 2, // We're acting as Party 2 (backup)
    };

    let mut reshare = ReshareSession::new(config, backup_share);

    // Generate our (Party 2) contribution
    let _our_messages = reshare.generate_round1()
        .map_err(|e| format!("recovery reshare round1 failed: {}", e))?;

    // Process server's reshare messages
    let mut messages = Vec::new();
    for msg_json in server_messages_json {
        let msg: ProtocolMessage = serde_json::from_str(&msg_json)
            .map_err(|e| format!("failed to deserialize server message: {}", e))?;
        messages.push(msg);
    }

    reshare.process_round1(messages)
        .map_err(|e| format!("recovery reshare process_round1 failed: {}", e))?;

    // Finalize to get the new device shard (Party 0)
    let new_device_share = reshare.finalize()
        .map_err(|e| format!("recovery reshare finalization failed: {}", e))?;

    // Verify the public key matches
    if new_device_share.public_key != public_key {
        return Err("recovery failed: public key mismatch".into());
    }

    let addr = new_device_share.eth_address();
    let address_hex = format!(
        "0x{}",
        addr.iter().map(|b| format!("{b:02x}")).collect::<String>()
    );

    // Store the recovered device shard as Party 0
    state::store_shares(vec![new_device_share]);

    // Clear the temporary backup shard from recovery state
    state::clear_recovery_backup_shard();

    Ok(FfiDkgComplete {
        address: address_hex,
        public_key,
    })
}

/// Clear the temporary backup shard from recovery state.
pub fn recovery_clear_backup_shard() {
    state::clear_recovery_backup_shard();
}

/// Check if a backup shard has been imported for recovery.
pub fn recovery_has_backup_shard() -> bool {
    state::get_recovery_backup_shard().is_some()
}

// ---------------------------------------------------------------------------
// Device Shard Export (for hardware-backed persistence)
// ---------------------------------------------------------------------------

/// Export the device shard (Party 0) secret share bytes for hardware-backed storage.
/// SECURITY: This should only be called once after DKG to persist to Secure Enclave/StrongBox.
/// The caller must immediately encrypt and store it via hardware security module.
pub fn export_device_shard() -> Result<Vec<u8>, String> {
    let share = state::get_share(0)
        .ok_or("device shard not loaded — DKG not complete")?;
    Ok(share.secret_share.as_bytes().to_vec())
}

/// Import a device shard (Party 0) from hardware-backed storage into Rust memory.
/// Called at app startup to restore the shard from Secure Enclave/StrongBox.
pub fn import_device_shard(shard_bytes: Vec<u8>, public_key: Vec<u8>) -> Result<(), String> {
    use k256::elliptic_curve::PrimeField;
    use k256::Scalar;

    if shard_bytes.len() != 32 {
        return Err(format!(
            "invalid device shard length: expected 32 bytes, got {}",
            shard_bytes.len()
        ));
    }

    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&shard_bytes);
    let _scalar = Option::<Scalar>::from(Scalar::from_repr(bytes.into()))
        .ok_or_else(|| "invalid device shard: not a valid secp256k1 scalar".to_string())?;

    let device_share = mpc_core::dkls23::KeyShare {
        party: 0,
        threshold: 2,
        total_parties: 3,
        secret_share: shard_bytes.into(),
        public_key,
        paillier_pk: None,
    };

    state::store_shares(vec![device_share]);
    Ok(())
}

// ---------------------------------------------------------------------------
// Backup Shard Verification
// ---------------------------------------------------------------------------

/// Verify a backup shard by combining it with the device shard via Lagrange
/// interpolation to reconstruct the group public key, then comparing against the expected key.
///
/// If `device_shard_bytes` is provided (non-empty), it is used directly.
/// Otherwise falls back to the device shard in memory (Party 0).
///
/// `expected_public_key` is the stored wallet public key to verify against.
pub fn verify_backup_shard(
    backup_bytes: Vec<u8>,
    device_shard_bytes: Vec<u8>,
    expected_public_key: Vec<u8>,
) -> Result<bool, String> {
    use k256::elliptic_curve::sec1::ToEncodedPoint;
    use k256::elliptic_curve::PrimeField;
    use k256::{ProjectivePoint, Scalar};

    if backup_bytes.len() != 32 {
        return Err(format!(
            "invalid backup shard length: expected 32 bytes, got {}",
            backup_bytes.len()
        ));
    }

    // Validate backup scalar
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&backup_bytes);
    let _scalar = Option::<Scalar>::from(Scalar::from_repr(bytes.into()))
        .ok_or_else(|| "invalid backup shard: not a valid secp256k1 scalar".to_string())?;

    // Get device shard bytes
    let device_bytes = if !device_shard_bytes.is_empty() {
        if device_shard_bytes.len() != 32 {
            return Err(format!(
                "invalid device shard length: expected 32 bytes, got {}",
                device_shard_bytes.len()
            ));
        }
        device_shard_bytes
    } else {
        // Fallback to in-memory device shard
        let device_share = state::get_share(0)
            .ok_or("device shard not loaded — provide device_shard_bytes or initialize wallet")?;
        device_share.secret_share.as_bytes().to_vec()
    };

    // Get expected public key
    let stored_pubkey = if !expected_public_key.is_empty() {
        expected_public_key
    } else {
        let device_share = state::get_share(0)
            .ok_or("no public key available — provide expected_public_key")?;
        if device_share.public_key.is_empty() {
            return Err("no stored public key to verify against".into());
        }
        device_share.public_key.clone()
    };

    // Reconstruct the group secret using Lagrange interpolation on parties {0, 2}
    let share_indices: Vec<u16> = vec![0, 2];
    let share_values: Vec<Vec<u8>> = vec![device_bytes, backup_bytes];

    let reconstructed_secret = mpc_core::dkls23::protocol::shamir_reconstruct(
        &share_indices,
        &share_values,
    )
    .map_err(|e| format!("Lagrange interpolation failed: {}", e))?;

    // Derive public key from reconstructed secret
    let secret_arr: [u8; 32] = reconstructed_secret
        .as_slice()
        .try_into()
        .map_err(|_| "invalid reconstructed secret length".to_string())?;

    let secret_scalar = Option::<Scalar>::from(Scalar::from_repr(secret_arr.into()))
        .ok_or_else(|| "reconstructed secret is not a valid scalar".to_string())?;

    let derived_pubkey_point = ProjectivePoint::GENERATOR * secret_scalar;
    let derived_pubkey = k256::PublicKey::from_affine(derived_pubkey_point.to_affine())
        .map_err(|e| format!("failed to derive public key: {}", e))?;

    // Compare as compressed (33 bytes) or uncompressed (65 bytes) depending on stored format
    let matches = if stored_pubkey.len() == 33 {
        derived_pubkey.to_encoded_point(true).as_bytes().to_vec() == stored_pubkey
    } else {
        derived_pubkey.to_encoded_point(false).as_bytes().to_vec() == stored_pubkey
    };

    Ok(matches)
}

// ---------------------------------------------------------------------------
// Noise_XX Transport Encryption (for WebSocket MPC messages)
// ---------------------------------------------------------------------------

pub struct FfiNoiseSession {
    /// Opaque session ID for referencing this Noise session in subsequent calls
    pub session_id: String,
}

pub struct FfiNoiseHandshakeResult {
    /// Base64-encoded handshake message to send to the peer
    pub message_base64: String,
    /// Whether the handshake is now complete (transport ready)
    pub is_ready: bool,
}

pub struct FfiNoiseKeypair {
    /// X25519 private key (32 bytes)
    pub private_key: Vec<u8>,
    /// X25519 public key (32 bytes)
    pub public_key: Vec<u8>,
}

/// Generate a new X25519 static keypair for Noise_XX.
/// The private key should be stored in secure storage (Keychain/Keystore).
pub fn noise_generate_keypair() -> Result<FfiNoiseKeypair, String> {
    let (priv_key, pub_key) = mpc_core::transport::noise::generate_keypair()
        .map_err(|e| format!("keypair generation failed: {}", e))?;
    Ok(FfiNoiseKeypair {
        private_key: priv_key,
        public_key: pub_key,
    })
}

/// Create a Noise_XX initiator session (device side) and generate the first handshake message.
/// `static_private_key` is the device's persistent X25519 private key (32 bytes).
/// Returns a session ID and the first handshake message (base64-encoded).
pub fn noise_initiator_start(static_private_key: Vec<u8>) -> Result<FfiNoiseHandshakeResult, String> {
    use mpc_core::transport::noise::NoiseSession;

    if static_private_key.len() != 32 {
        return Err(format!("static key must be 32 bytes, got {}", static_private_key.len()));
    }

    let mut session = NoiseSession::new_initiator(&static_private_key)
        .map_err(|e| format!("failed to create initiator: {}", e))?;

    // Generate first message (-> e)
    let msg1 = session.handshake_step(&[])
        .map_err(|e| format!("handshake step 1 failed: {}", e))?;

    let session_id = uuid::Uuid::new_v4().to_string();
    state::create_noise_session(session_id.clone(), session);

    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
    Ok(FfiNoiseHandshakeResult {
        message_base64: BASE64.encode(&msg1),
        is_ready: false,
    })
}

/// Process a handshake message from the server and generate the next message.
/// For the initiator, this is called once with the server's response (step 2),
/// producing the final handshake message (step 3). After this call, `is_ready` is true.
pub fn noise_initiator_finish(
    session_id: String,
    server_message_base64: String,
) -> Result<FfiNoiseHandshakeResult, String> {
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

    let server_msg = BASE64.decode(&server_message_base64)
        .map_err(|e| format!("invalid base64: {}", e))?;

    let arc = state::get_noise_session_arc(&session_id)
        .ok_or("noise session not found")?;

    let mut session = arc.lock().unwrap();
    let response = session.handshake_step(&server_msg)
        .map_err(|e| format!("handshake step failed: {}", e))?;

    let is_ready = session.is_transport_ready();

    Ok(FfiNoiseHandshakeResult {
        message_base64: BASE64.encode(&response),
        is_ready,
    })
}

/// Encrypt a plaintext message using the established Noise session.
/// Returns base64-encoded ciphertext.
/// Only valid after the handshake is complete (`is_ready` was true).
pub fn noise_encrypt(session_id: String, plaintext: Vec<u8>) -> Result<String, String> {
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

    let arc = state::get_noise_session_arc(&session_id)
        .ok_or("noise session not found")?;

    let mut session = arc.lock().unwrap();
    let ciphertext = session.encrypt(&plaintext)
        .map_err(|e| format!("encryption failed: {}", e))?;

    Ok(BASE64.encode(&ciphertext))
}

/// Decrypt a ciphertext message using the established Noise session.
/// `ciphertext_base64` is the base64-encoded ciphertext from the server.
/// Returns the decrypted plaintext bytes.
pub fn noise_decrypt(session_id: String, ciphertext_base64: String) -> Result<Vec<u8>, String> {
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

    let arc = state::get_noise_session_arc(&session_id)
        .ok_or("noise session not found")?;

    let ciphertext = BASE64.decode(&ciphertext_base64)
        .map_err(|e| format!("invalid base64: {}", e))?;

    let mut session = arc.lock().unwrap();
    let plaintext = session.decrypt(&ciphertext)
        .map_err(|e| format!("decryption failed: {}", e))?;

    Ok(plaintext)
}

/// Get the remote peer's static public key (after handshake completes).
/// Returns the 32-byte X25519 public key, or an error if not available.
pub fn noise_get_remote_public_key(session_id: String) -> Result<Vec<u8>, String> {
    let arc = state::get_noise_session_arc(&session_id)
        .ok_or("noise session not found")?;

    let session = arc.lock().unwrap();
    session.remote_static_key()
        .map(|k| k.to_vec())
        .ok_or_else(|| "remote public key not available (handshake not complete)".to_string())
}

/// Destroy a Noise session and free its resources.
pub fn noise_session_destroy(session_id: String) {
    state::delete_noise_session(&session_id);
}

// ---------------------------------------------------------------------------
// Legacy signing (for testing)
// ---------------------------------------------------------------------------

/// Legacy: Sign locally for testing (reconstructs full key — NOT for production).
pub fn sign_hash(msg_hash: Vec<u8>) -> Result<Vec<u8>, String> {
    if msg_hash.len() != 32 {
        return Err("msg_hash must be 32 bytes".into());
    }

    let share0 = state::get_share(0).ok_or("device shard not loaded")?;
    let share1 = state::get_share(1).ok_or("server shard not loaded")?;

    let indices = vec![share0.party, share1.party];

    let msg_arr: [u8; 32] = msg_hash.try_into().map_err(|_| "msg_hash must be 32 bytes")?;

    let (sig_bytes, recovery_id) = mpc_core::dkls23::protocol::threshold_sign(
        &indices,
        &[&share0, &share1],
        &msg_arr,
    )
    .map_err(|e| e.to_string())?;

    let mut result = sig_bytes;
    result.push(recovery_id);
    Ok(result)
}

// ---------------------------------------------------------------------------
// Balance queries (async, uses tokio)
// ---------------------------------------------------------------------------

/// Query ETH balance for an address on Base Sepolia.
/// Returns balance in wei as a decimal string.
pub async fn query_eth_balance(address: String, rpc_url: String) -> Result<FfiBalance, String> {
    let owner_addr: Address = address
        .parse()
        .map_err(|e| format!("Invalid address: {}", e))?;

    let balance = chain_evm::tokens::query_native_balance(owner_addr, &rpc_url)
        .await
        .map_err(|e| format!("RPC error: {}", e))?;

    let wei_str = balance.to_string();

    // Format for display (simplified: divide by 1e18)
    let eth_value = balance
        .checked_div(alloy_primitives::U256::from(1_000_000_000_000_000u64))
        .unwrap_or_default();
    let formatted = format!("{} ETH", eth_value);

    Ok(FfiBalance {
        wei: wei_str,
        formatted,
        decimals: 18,
    })
}

/// Query ERC-20 token balance.
pub async fn query_token_balance(
    owner: String,
    token_contract: String,
    rpc_url: String,
) -> Result<FfiBalance, String> {
    let owner_addr: Address = owner
        .parse()
        .map_err(|e| format!("Invalid owner address: {}", e))?;

    let token_addr: Address = token_contract
        .parse()
        .map_err(|e| format!("Invalid token address: {}", e))?;

    let balance = chain_evm::tokens::query_balance(token_addr, owner_addr, &rpc_url)
        .await
        .map_err(|e| format!("RPC error: {}", e))?;

    let wei_str = balance.to_string();

    // Default to 6 decimals for USDC-style formatting
    let divisor = U256::from(1_000_000u64);
    let display_value = balance
        .checked_div(divisor)
        .unwrap_or_default();
    let formatted = format!("{} tokens", display_value);

    Ok(FfiBalance {
        wei: wei_str,
        formatted,
        decimals: 6,
    })
}

// ---------------------------------------------------------------------------
// Transaction building
// ---------------------------------------------------------------------------

/// Estimate gas for a transaction.
pub async fn estimate_gas(
    _from: String,
    _to: String,
    _value_wei: String,
    _data: Option<Vec<u8>>,
    _rpc_url: String,
) -> Result<FfiGasEstimate, String> {
    // Use hardcoded gas estimates (simplified for FFI)
    // In production, this would call an RPC endpoint
    let base_fee: u128 = 1_000_000_000; // 1 gwei
    let priority_fee: u128 = 100_000_000; // 0.1 gwei

    let gas_estimate = chain_evm::gas::estimate_gas(
        chain_evm::chains::GasModel::OpBedrock, // Base uses Optimism Bedrock
        false,                                  // not ERC-20
        base_fee,
        priority_fee,
        None,
    );

    let value: u128 = _value_wei.parse().unwrap_or(0);
    let max_fee = gas_estimate
        .max_fee_per_gas
        .unwrap_or(base_fee * 2 + priority_fee);
    let estimated_cost = value + (gas_estimate.gas_limit as u128) * max_fee;

    Ok(FfiGasEstimate {
        gas_limit: gas_estimate.gas_limit,
        max_fee_per_gas: max_fee.to_string(),
        max_priority_fee_per_gas: gas_estimate
            .max_priority_fee_per_gas
            .unwrap_or(priority_fee)
            .to_string(),
        estimated_cost_wei: estimated_cost.to_string(),
    })
}

/// Build, sign, and broadcast an ETH transfer.
/// Biometric auth must happen on the Dart side BEFORE calling this.
pub async fn send_eth(
    to: String,
    value_wei: String,
    chain_id: u64,
    rpc_url: String,
) -> Result<FfiTxResult, String> {
    let shares = get_signing_shares()?;
    let to_addr: Address = to.parse().map_err(|e| format!("Invalid to address: {}", e))?;
    let value = U256::from_str_radix(&value_wei, 10).map_err(|e| format!("Invalid value: {}", e))?;
    let eth_addr_bytes = shares[0].eth_address();
    let sender_addr = Address::from_slice(&eth_addr_bytes);
    let client = reqwest::Client::new();

    let nonce = chain_evm::transaction::get_nonce(&client, &rpc_url, sender_addr)
        .await
        .map_err(|e| format!("Failed to fetch nonce: {}", e))?;

    let gas_estimate = default_gas_estimate(21000);

    let tx_request = chain_evm::transaction::TransactionRequest {
        to: to_addr,
        value,
        data: Vec::new(),
        chain_id,
        gas_limit: Some(21000),
        nonce: Some(nonce),
    };

    let signer = chain_evm::signer::MpcSigner::from_shares(
        sender_addr, chain_id, vec![0, 1], shares,
    );

    let (encoded, _) = chain_evm::transaction::sign_eip1559_tx(
        &tx_request, &gas_estimate, nonce, &signer,
    ).map_err(|e| format!("Signing failed: {:?}", e))?;

    let tx_hash = chain_evm::transaction::broadcast_tx(&client, &rpc_url, &encoded)
        .await
        .map_err(|e| format!("Broadcast failed: {}", e))?;

    Ok(FfiTxResult {
        tx_hash: format!("0x{}", hex::encode(tx_hash.as_slice())),
    })
}

/// Build, sign, and broadcast an ERC-20 token transfer.
pub async fn send_erc20(
    to: String,
    token_contract: String,
    amount_raw: String,
    chain_id: u64,
    rpc_url: String,
) -> Result<FfiTxResult, String> {
    let shares = get_signing_shares()?;
    let to_addr: Address = to.parse().map_err(|e| format!("Invalid to address: {}", e))?;
    let contract_addr: Address = token_contract.parse().map_err(|e| format!("Invalid token contract: {}", e))?;
    let amount = U256::from_str_radix(&amount_raw, 10).map_err(|e| format!("Invalid amount: {}", e))?;
    let eth_addr_bytes = shares[0].eth_address();
    let sender_addr = Address::from_slice(&eth_addr_bytes);
    let client = reqwest::Client::new();

    let nonce = chain_evm::transaction::get_nonce(&client, &rpc_url, sender_addr)
        .await
        .map_err(|e| format!("Failed to fetch nonce: {}", e))?;

    // ERC-20 transfer(address,uint256) calldata
    let mut calldata = vec![0xa9, 0x05, 0x9c, 0xbb]; // selector
    calldata.extend_from_slice(&[0u8; 12]); // pad address to 32 bytes
    calldata.extend_from_slice(to_addr.as_slice());
    let mut amount_bytes = [0u8; 32];
    amount.to_be_bytes::<32>().iter().enumerate().for_each(|(i, &b)| amount_bytes[i] = b);
    calldata.extend_from_slice(&amount_bytes);

    let gas_estimate = default_gas_estimate(65000);

    let tx_request = chain_evm::transaction::TransactionRequest {
        to: contract_addr,
        value: U256::ZERO,
        data: calldata,
        chain_id,
        gas_limit: Some(65000),
        nonce: Some(nonce),
    };

    let signer = chain_evm::signer::MpcSigner::from_shares(
        sender_addr, chain_id, vec![0, 1], shares,
    );

    let (encoded, _) = chain_evm::transaction::sign_eip1559_tx(
        &tx_request, &gas_estimate, nonce, &signer,
    ).map_err(|e| format!("Signing failed: {:?}", e))?;

    let tx_hash = chain_evm::transaction::broadcast_tx(&client, &rpc_url, &encoded)
        .await
        .map_err(|e| format!("Broadcast failed: {}", e))?;

    Ok(FfiTxResult {
        tx_hash: format!("0x{}", hex::encode(tx_hash.as_slice())),
    })
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn get_signing_shares() -> Result<Vec<mpc_core::dkls23::KeyShare>, String> {
    if !state::has_shares() {
        return Err("No wallet found — generate wallet first".into());
    }
    let share0 = state::get_share(0).ok_or("Share 0 not found")?;
    let share1 = state::get_share(1).ok_or("Share 1 not found")?;
    Ok(vec![share0, share1])
}

fn default_gas_estimate(gas_limit: u64) -> chain_evm::transaction::GasEstimate {
    let max_fee: u128 = 2_000_000_000; // 2 gwei
    let max_priority_fee: u128 = 100_000_000; // 0.1 gwei
    chain_evm::transaction::GasEstimate {
        gas_limit,
        max_fee_per_gas: max_fee,
        max_priority_fee_per_gas: max_priority_fee,
        l1_data_fee: None,
        estimated_cost_wei: U256::from(gas_limit) * U256::from(max_fee),
        estimated_cost_usd: None,
    }
}
