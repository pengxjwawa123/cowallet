pub mod shard_store;
pub mod types;

use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use mpc_core::dkls23::dkg::DkgSession;
use mpc_core::dkls23::reshare::ReshareSession;
use mpc_core::dkls23::sign::SignSession;
use mpc_core::dkls23::{KeyShare, ProtocolMessage, SessionConfig};
use sqlx::PgPool;
use tokio::sync::Notify;
use uuid::Uuid;

use crate::services::crypto::EncryptionService;
use crate::services::presign_manager::{PresignManager, PresignatureData};

use self::shard_store::ShardStore;
use self::types::*;

const SESSION_TIMEOUT: Duration = Duration::from_secs(300);

/// Server-side MPC participant that automatically processes protocol rounds as Party 1.
///
/// Lifecycle:
/// 1. When a new MPC session is created, `on_session_created` initializes the server's
///    protocol state and generates its Round 1 message.
/// 2. When client (Party 0) sends a message to Party 1, `on_message_received` advances
///    the state machine and generates response messages.
/// 3. On DKG completion, the server's KeyShare is encrypted and stored.
/// 4. On Sign, the stored KeyShare is loaded and used for the signing protocol.
pub struct MpcParticipant {
    shard_store: Arc<ShardStore>,
    db: PgPool,
    dkg_sessions: Arc<DashMap<Uuid, DkgSession>>,
    sign_sessions: Arc<DashMap<Uuid, SignSession>>,
    reshare_sessions: Arc<DashMap<Uuid, ReshareSession>>,
    session_meta: Arc<DashMap<Uuid, ActiveSession>>,
    /// Cached presignature data per session (reserved during init_sign_session).
    reserved_presignatures: Arc<DashMap<Uuid, PresignatureData>>,
    /// Optional presign manager for pre-computing signing material.
    presign_manager: Option<Arc<PresignManager>>,
    shutdown: Arc<Notify>,
}

impl MpcParticipant {
    pub fn new(db: PgPool, encryption: EncryptionService) -> Self {
        let shard_store = Arc::new(ShardStore::new(db.clone(), encryption));
        Self {
            shard_store,
            db,
            dkg_sessions: Arc::new(DashMap::new()),
            sign_sessions: Arc::new(DashMap::new()),
            reshare_sessions: Arc::new(DashMap::new()),
            session_meta: Arc::new(DashMap::new()),
            reserved_presignatures: Arc::new(DashMap::new()),
            presign_manager: None,
            shutdown: Arc::new(Notify::new()),
        }
    }

    /// Set the presign manager for this participant.
    /// Called after both are initialized in AppState.
    pub fn set_presign_manager(&mut self, mgr: Arc<PresignManager>) {
        self.presign_manager = Some(mgr);
    }

    /// Start background cleanup task for expired sessions.
    pub fn spawn_cleanup(self: &Arc<Self>) {
        let this = Arc::clone(self);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        this.cleanup_expired();
                    }
                    _ = this.shutdown.notified() => break,
                }
            }
        });
    }

    /// Called when a new MPC session is created via HTTP.
    /// The server (Party 1) initializes its protocol state and generates Round 1.
    pub async fn on_session_created(
        &self,
        session_id: Uuid,
        user_id: Uuid,
        session_type: &str,
        parties: &[i16],
        threshold: i16,
        wallet_id: Option<Uuid>,
    ) -> Result<(), String> {
        let mpc_type = MpcSessionType::from_str(session_type)
            .ok_or_else(|| format!("unsupported session type: {}", session_type))?;

        // Only participate if Party 1 is in the party list
        if !parties.contains(&(SERVER_PARTY_INDEX as i16)) {
            tracing::debug!("Session {} does not include server party, skipping", session_id);
            return Ok(());
        }

        let config = SessionConfig {
            session_id: session_id.to_string(),
            threshold: threshold as u16,
            total_parties: parties.len() as u16,
            party_index: SERVER_PARTY_INDEX,
        };

        match mpc_type {
            MpcSessionType::Dkg | MpcSessionType::Keygen => {
                self.init_dkg_session(session_id, user_id, config).await
            }
            MpcSessionType::Sign => {
                self.init_sign_session(session_id, user_id, config, wallet_id).await
            }
            MpcSessionType::Reshare => {
                self.init_reshare_session(session_id, user_id, config).await
            }
        }
    }

    /// Called when a message addressed to Party 1 is stored.
    /// Processes the message and generates a response.
    pub async fn on_message_received(
        &self,
        session_id: Uuid,
        from_party: i16,
        round: i16,
        payload: &[u8],
    ) -> Result<Vec<(i16, i16, Vec<u8>)>, String> {
        let meta = self.session_meta.get(&session_id)
            .ok_or_else(|| format!("no active session {}", session_id))?;
        let session_type = meta.session_type;
        let user_id = meta.user_id;
        drop(meta);

        match session_type {
            MpcSessionType::Dkg | MpcSessionType::Keygen => {
                self.process_dkg_message(session_id, user_id, from_party, round, payload).await
            }
            MpcSessionType::Sign => {
                self.process_sign_message(session_id, user_id, from_party, round, payload).await
            }
            MpcSessionType::Reshare => {
                self.process_reshare_message(session_id, user_id, from_party, round, payload).await
            }
        }
    }

    /// Initialize a DKG session: create the session and generate server's Round 1.
    async fn init_dkg_session(
        &self,
        session_id: Uuid,
        user_id: Uuid,
        config: SessionConfig,
    ) -> Result<(), String> {
        let mut dkg = DkgSession::new(config.clone());

        // Generate server's Round 1 message immediately
        let round1_msg = dkg.generate_round1()
            .map_err(|e| format!("DKG round 1 generation failed: {}", e))?;

        // Store the server's Round 1 message in DB so client can poll it
        self.store_outbound_message(
            session_id,
            SERVER_PARTY_INDEX as i16,
            0, // to client (Party 0) — broadcast actually
            1,
            &round1_msg.payload,
        ).await?;

        self.dkg_sessions.insert(session_id, dkg);
        self.session_meta.insert(session_id, ActiveSession {
            session_id,
            user_id,
            session_type: MpcSessionType::Dkg,
            phase: SessionPhase::AwaitingClientRound1,
            config,
            created_at: Instant::now(),
            wallet_id: None,
        });

        tracing::info!("Server DKG session {} initialized, Round 1 sent", session_id);
        Ok(())
    }

    /// Initialize a Sign session: store metadata and wait for client's Round 1.
    /// The message hash arrives with the client's first message payload.
    async fn init_sign_session(
        &self,
        session_id: Uuid,
        user_id: Uuid,
        config: SessionConfig,
        wallet_id: Option<Uuid>,
    ) -> Result<(), String> {
        // Verify server shard exists before accepting the session
        // Use wallet-specific shard if wallet_id is provided
        if let Some(wid) = wallet_id {
            let _key_share = self.shard_store.load_key_share_for_wallet(user_id, wid).await?
                .ok_or_else(|| format!("no server shard for user {} wallet {}, DKG must complete first", user_id, wid))?;
        } else {
            let _key_share = self.shard_store.load_key_share(user_id).await?
                .ok_or_else(|| format!("no server shard for user {}, DKG must complete first", user_id))?;
        }

        // Try to reserve a presignature for this signing session.
        // If available, the pre-computed k_1 can be used instead of generating fresh
        // randomness during Round 1, reducing online signing latency.
        if let (Some(wid), Some(presign_mgr)) = (wallet_id, &self.presign_manager) {
            match presign_mgr.reserve_presignature(wid, session_id).await {
                Ok(Some(presig_data)) => {
                    tracing::info!(
                        "Reserved presignature {} for sign session {} (wallet {})",
                        presig_data.id, session_id, wid
                    );
                    self.reserved_presignatures.insert(session_id, presig_data);
                }
                Ok(None) => {
                    tracing::debug!(
                        "No presignature available for wallet {}, will generate fresh k during Round 1",
                        wid
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to reserve presignature for session {}: {} — proceeding without",
                        session_id, e
                    );
                }
            }
        }

        self.session_meta.insert(session_id, ActiveSession {
            session_id,
            user_id,
            session_type: MpcSessionType::Sign,
            phase: SessionPhase::SignAwaitingRound1,
            config,
            created_at: Instant::now(),
            wallet_id,
        });

        tracing::info!("Server Sign session {} initialized (wallet: {:?}), awaiting client Round 1", session_id, wallet_id);
        Ok(())
    }

    /// Process an inbound DKG message from the client.
    async fn process_dkg_message(
        &self,
        session_id: Uuid,
        user_id: Uuid,
        _from_party: i16,
        round: i16,
        payload: &[u8],
    ) -> Result<Vec<(i16, i16, Vec<u8>)>, String> {
        let mut dkg = self.dkg_sessions.get_mut(&session_id)
            .ok_or("DKG session not found")?;

        let incoming = ProtocolMessage {
            session_id: session_id.to_string(),
            from: 0, // client is party 0
            to: SERVER_PARTY_INDEX,
            round: round as u16,
            payload: payload.to_vec(),
        };

        let mut outbound = Vec::new();

        match round {
            1 => {
                // Client sent their Round 1 (commitments)
                dkg.process_round1(vec![incoming])
                    .map_err(|e| format!("process_round1 failed: {}", e))?;

                // Generate server's Round 2 messages
                let round2_msgs = dkg.generate_round2()
                    .map_err(|e| format!("generate_round2 failed: {}", e))?;

                for msg in round2_msgs {
                    // Only send messages addressed to Party 0 (client)
                    if msg.to == 0 || msg.to == BROADCAST_PARTY {
                        outbound.push((SERVER_PARTY_INDEX as i16, msg.to as i16, msg.payload));
                    }
                }

                if let Some(mut meta) = self.session_meta.get_mut(&session_id) {
                    meta.phase = SessionPhase::AwaitingClientRound2;
                }

                tracing::info!("DKG session {}: processed client R1, sent server R2", session_id);
            }
            2 => {
                // Client sent their Round 2 (share evaluations)
                let key_share = dkg.process_round2(vec![incoming])
                    .map_err(|e| format!("process_round2 failed: {}", e))?;

                // Compute eth_address from the KeyShare's public key
                let eth_addr = key_share.eth_address();

                // Count existing wallets for naming
                let wallet_count: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*) FROM wallets WHERE user_id = $1"
                )
                .bind(user_id)
                .fetch_one(&self.db)
                .await
                .unwrap_or(0);

                let wallet_name = format!("Wallet {}", wallet_count + 1);
                let default_chain_ids: Vec<i64> = vec![84532]; // Base Sepolia

                // Create wallet entry in the wallets table
                let wallet_id: Uuid = sqlx::query_scalar(
                    "INSERT INTO wallets (user_id, name, public_key, eth_address, chain_ids, status)
                     VALUES ($1, $2, $3, $4, $5, 'active')
                     RETURNING id"
                )
                .bind(user_id)
                .bind(&wallet_name)
                .bind(&key_share.public_key)
                .bind(&eth_addr.as_slice())
                .bind(&default_chain_ids)
                .fetch_one(&self.db)
                .await
                .map_err(|e| format!("failed to create wallet entry: {}", e))?;

                // Store the server's encrypted KeyShare with wallet_id association
                self.shard_store.store_key_share_for_wallet(user_id, wallet_id, &key_share).await?;

                if let Some(mut meta) = self.session_meta.get_mut(&session_id) {
                    meta.phase = SessionPhase::DkgComplete;
                }

                // Update session status and wallet_id in DB
                let _ = sqlx::query(
                    "UPDATE mpc_sessions SET status = 'completed', completed_at = NOW(), wallet_id = $2 WHERE id = $1"
                )
                .bind(session_id)
                .bind(wallet_id)
                .execute(&self.db)
                .await;

                // Clean up in-memory state
                drop(dkg);
                self.dkg_sessions.remove(&session_id);

                tracing::info!(
                    "DKG session {} COMPLETE. Wallet {} created, server shard stored for user {}",
                    session_id, wallet_id, user_id
                );
            }
            _ => {
                return Err(format!("unexpected DKG round {}", round));
            }
        }

        // Store outbound messages in DB
        for (from, to, ref payload) in &outbound {
            self.store_outbound_message(session_id, *from, *to, round + 1, payload).await?;
        }

        Ok(outbound)
    }

    /// Process an inbound Sign message from the client.
    ///
    /// Protocol flow (server is higher-indexed Party 1):
    /// Round 1: Receive client R_0 + msg_hash → create SignSession → process R_0 → generate R_1
    /// Round 2: Receive MtARequest → process_round2 (homomorphic Enc(s)) → send ServerSignature(Enc(s))
    async fn process_sign_message(
        &self,
        session_id: Uuid,
        user_id: Uuid,
        _from_party: i16,
        round: i16,
        payload: &[u8],
    ) -> Result<Vec<(i16, i16, Vec<u8>)>, String> {
        let mut outbound = Vec::new();

        match round {
            1 => {
                // Client Round 1 contains both R_0 (serialized SignRound1Message) and msg_hash.
                // The msg_hash is embedded in the session context sent by the client.
                // We extract it from the JSON wrapper the client sends.
                let msg_hash = self.extract_msg_hash(payload)?;

                // Load server's key share and create the actual SignSession now
                let meta = self.session_meta.get(&session_id)
                    .ok_or("session meta not found")?;
                let config = meta.config.clone();
                let wallet_id = meta.wallet_id;
                drop(meta);

                // Use wallet-specific shard if wallet_id is available
                let key_share = if let Some(wid) = wallet_id {
                    self.shard_store.load_key_share_for_wallet(user_id, wid).await?
                        .ok_or_else(|| format!("no server shard for user {} wallet {}", user_id, wid))?
                } else {
                    self.shard_store.load_key_share(user_id).await?
                        .ok_or_else(|| format!("no server shard for user {}", user_id))?
                };

                let mut sign = SignSession::new_distributed(config, key_share, msg_hash);

                // Generate server's R_1 first (must happen before process_round1)
                let server_r1 = sign.generate_round1()
                    .map_err(|e| format!("sign generate_round1 failed: {}", e))?;

                // Now process client's R_0
                let incoming = ProtocolMessage {
                    session_id: session_id.to_string(),
                    from: 0,
                    to: SERVER_PARTY_INDEX,
                    round: 1,
                    payload: payload.to_vec(),
                };
                sign.process_round1(vec![incoming])
                    .map_err(|e| format!("sign process_round1 failed: {}", e))?;

                // Send R_1 back to client
                outbound.push((SERVER_PARTY_INDEX as i16, 0i16, server_r1.payload));

                // Store the session for round 2
                self.sign_sessions.insert(session_id, sign);

                if let Some(mut meta) = self.session_meta.get_mut(&session_id) {
                    meta.phase = SessionPhase::SignAwaitingRound2;
                }

                tracing::info!("Sign session {}: processed client R1, sent server R1", session_id);
            }
            2 => {
                // Client sent MtARequest (Paillier-encrypted values + range proofs)
                let mut sign = self.sign_sessions.get_mut(&session_id)
                    .ok_or("Sign session not found")?;

                let incoming = ProtocolMessage {
                    session_id: session_id.to_string(),
                    from: 0,
                    to: SERVER_PARTY_INDEX,
                    round: 2,
                    payload: payload.to_vec(),
                };

                // Server computes Enc(s) homomorphically and stores ServerSignature internally
                let _placeholder = sign.process_round2(vec![incoming])
                    .map_err(|e| format!("sign process_round2 failed: {}", e))?;

                // Extract the actual ServerSignature (contains Enc(s) ciphertext)
                let server_sig_payload = sign.get_server_response()
                    .ok_or_else(|| "server did not produce ServerSignature".to_string())?;

                outbound.push((SERVER_PARTY_INDEX as i16, 0i16, server_sig_payload));

                if let Some(mut meta) = self.session_meta.get_mut(&session_id) {
                    meta.phase = SessionPhase::SignComplete;
                }

                let _ = sqlx::query(
                    "UPDATE mpc_sessions SET status = 'completed', completed_at = NOW() WHERE id = $1"
                )
                .bind(session_id)
                .execute(&self.db)
                .await;

                drop(sign);
                self.sign_sessions.remove(&session_id);

                // Mark the reserved presignature as consumed (if one was used)
                if let Some((_, presig_data)) = self.reserved_presignatures.remove(&session_id) {
                    if let Some(presign_mgr) = &self.presign_manager {
                        if let Err(e) = presign_mgr.consume_presignature(presig_data.id).await {
                            tracing::warn!("Failed to mark presignature {} as consumed: {}", presig_data.id, e);
                        }
                    }
                }

                tracing::info!("Sign session {} COMPLETE", session_id);
            }
            _ => {
                return Err(format!("unexpected sign round {}", round));
            }
        }

        // Store outbound messages
        for (i, (from, to, ref msg_payload)) in outbound.iter().enumerate() {
            let out_round = round + i as i16;
            self.store_outbound_message(session_id, *from, *to, out_round, msg_payload).await?;
        }

        Ok(outbound)
    }

    /// Initialize a Reshare session: load existing key share, create session, generate Round 1.
    async fn init_reshare_session(
        &self,
        session_id: Uuid,
        user_id: Uuid,
        config: SessionConfig,
    ) -> Result<(), String> {
        // Load existing server key share — resharing requires the old share
        let key_share = self.shard_store.load_key_share(user_id).await?
            .ok_or_else(|| format!("no server shard for user {}, cannot reshare without existing share", user_id))?;

        let mut reshare = ReshareSession::new(config.clone(), key_share);

        // Generate server's Round 1 messages (polynomial evaluations for each party)
        let round1_msgs = reshare.generate_round1()
            .map_err(|e| format!("Reshare round 1 generation failed: {}", e))?;

        // Store outbound messages addressed to client (Party 0)
        for msg in &round1_msgs {
            if msg.to == 0 || msg.to == BROADCAST_PARTY {
                self.store_outbound_message(
                    session_id,
                    SERVER_PARTY_INDEX as i16,
                    msg.to as i16,
                    1,
                    &msg.payload,
                ).await?;
            }
        }

        self.reshare_sessions.insert(session_id, reshare);
        self.session_meta.insert(session_id, ActiveSession {
            session_id,
            user_id,
            session_type: MpcSessionType::Reshare,
            phase: SessionPhase::ReshareAwaitingRound1,
            config,
            created_at: Instant::now(),
            wallet_id: None,
        });

        tracing::info!("Server Reshare session {} initialized, Round 1 sent", session_id);
        Ok(())
    }

    /// Process an inbound Reshare message from the client.
    ///
    /// Protocol flow:
    /// Round 1: Receive client's polynomial evaluations → process_round1 → finalize → store new share
    async fn process_reshare_message(
        &self,
        session_id: Uuid,
        user_id: Uuid,
        _from_party: i16,
        round: i16,
        payload: &[u8],
    ) -> Result<Vec<(i16, i16, Vec<u8>)>, String> {
        let outbound = Vec::new();

        match round {
            1 => {
                let mut reshare = self.reshare_sessions.get_mut(&session_id)
                    .ok_or("Reshare session not found")?;

                let incoming = ProtocolMessage {
                    session_id: session_id.to_string(),
                    from: 0, // client is party 0
                    to: SERVER_PARTY_INDEX,
                    round: 1,
                    payload: payload.to_vec(),
                };

                // Process client's round 1 (their polynomial evaluations for us)
                reshare.process_round1(vec![incoming])
                    .map_err(|e| format!("reshare process_round1 failed: {}", e))?;

                // Finalize to get the new key share
                let new_key_share = reshare.finalize()
                    .map_err(|e| format!("reshare finalize failed: {}", e))?;

                // Store the new key share (upsert replaces the old one)
                drop(reshare);
                self.shard_store.store_key_share(user_id, &new_key_share).await?;

                if let Some(mut meta) = self.session_meta.get_mut(&session_id) {
                    meta.phase = SessionPhase::ReshareComplete;
                }

                // Update session status in DB
                let _ = sqlx::query(
                    "UPDATE mpc_sessions SET status = 'completed', completed_at = NOW() WHERE id = $1"
                )
                .bind(session_id)
                .execute(&self.db)
                .await;

                // Clean up in-memory state
                self.reshare_sessions.remove(&session_id);

                tracing::info!("Reshare session {} COMPLETE. New server shard stored for user {}", session_id, user_id);
            }
            _ => {
                return Err(format!("unexpected reshare round {}", round));
            }
        }

        Ok(outbound)
    }

    /// Extract the 32-byte message hash from the client's Round 1 payload.
    /// Client sends a JSON wrapper: {"msg_hash": [...], "party": 0, "timestamp": ...}
    /// OR directly sends the serialized SignRound1Message with msg_hash appended.
    fn extract_msg_hash(&self, payload: &[u8]) -> Result<[u8; 32], String> {
        // Try JSON parse first (mobile client sends JSON)
        if let Ok(json) = serde_json::from_slice::<serde_json::Value>(payload) {
            if let Some(hash_arr) = json.get("msg_hash").and_then(|v| v.as_array()) {
                let bytes: Vec<u8> = hash_arr.iter()
                    .filter_map(|v| v.as_u64().map(|n| n as u8))
                    .collect();
                if bytes.len() == 32 {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&bytes);
                    return Ok(arr);
                }
            }
        }

        // Fallback: try bincode deserialization of SignRound1Message
        // The msg_hash might be appended after the round1 message
        if payload.len() > 32 {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&payload[payload.len() - 32..]);
            return Ok(arr);
        }

        Err("could not extract msg_hash from client round 1 payload".into())
    }

    /// Persist an outbound message from the server into mpc_messages so the client can poll it.
    async fn store_outbound_message(
        &self,
        session_id: Uuid,
        from_party: i16,
        to_party: i16,
        round: i16,
        payload: &[u8],
    ) -> Result<(), String> {
        sqlx::query(
            "INSERT INTO mpc_messages (session_id, from_party, to_party, round, payload, verified)
             VALUES ($1, $2, $3, $4, $5, true)"
        )
        .bind(session_id)
        .bind(from_party)
        .bind(to_party)
        .bind(round)
        .bind(payload)
        .execute(&self.db)
        .await
        .map_err(|e| format!("failed to store outbound message: {}", e))?;

        // Update session activity
        let _ = sqlx::query(
            "UPDATE mpc_sessions SET last_activity = NOW(), current_round = GREATEST(current_round, $2)
             WHERE id = $1"
        )
        .bind(session_id)
        .bind(round as i32)
        .execute(&self.db)
        .await;

        Ok(())
    }

    /// Remove expired sessions from memory.
    fn cleanup_expired(&self) {
        let mut expired = Vec::new();
        for entry in self.session_meta.iter() {
            if entry.created_at.elapsed() > SESSION_TIMEOUT {
                expired.push(*entry.key());
            }
        }
        for id in expired {
            self.session_meta.remove(&id);
            self.dkg_sessions.remove(&id);
            self.sign_sessions.remove(&id);
            self.reshare_sessions.remove(&id);
            // Clean up any reserved presignatures for expired sessions
            self.reserved_presignatures.remove(&id);
            tracing::debug!("Cleaned up expired server session {}", id);
        }
    }

    /// Graceful shutdown.
    pub fn shutdown(&self) {
        self.shutdown.notify_one();
    }
}
