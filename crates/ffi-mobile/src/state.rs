use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use mpc_core::dkls23::dkg::DkgSession;
use mpc_core::dkls23::presign::PresignSession;
use mpc_core::dkls23::reshare::ReshareSession;
use mpc_core::dkls23::sign::SignSession;
use mpc_core::dkls23::{KeyShare, PartyIndex};
use mpc_core::transport::noise::NoiseSession;

static SHARDS: Mutex<Option<HashMap<PartyIndex, KeyShare>>> = Mutex::new(None);
static DKG_SESSIONS: std::sync::LazyLock<Mutex<HashMap<String, Arc<Mutex<DkgSession>>>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));
static SIGN_SESSIONS: std::sync::LazyLock<Mutex<HashMap<String, Arc<Mutex<SignSession>>>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));
static PRESIGN_SESSIONS: std::sync::LazyLock<Mutex<HashMap<String, Arc<Mutex<PresignSession>>>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));
static RESHARE_SESSIONS: std::sync::LazyLock<Mutex<HashMap<String, Arc<Mutex<ReshareSession>>>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));
static NOISE_SESSIONS: std::sync::LazyLock<Mutex<HashMap<String, Arc<Mutex<NoiseSession>>>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn store_shares(shares: Vec<KeyShare>) {
    let mut map = HashMap::new();
    for s in shares {
        map.insert(s.party, s);
    }
    *SHARDS.lock().unwrap() = Some(map);
}

pub fn get_share(party: PartyIndex) -> Option<KeyShare> {
    SHARDS.lock().unwrap().as_ref()?.get(&party).cloned()
}

pub fn clear_shares() {
    *SHARDS.lock().unwrap() = None;
}

pub fn has_shares() -> bool {
    SHARDS
        .lock()
        .unwrap()
        .as_ref()
        .is_some_and(|m| !m.is_empty())
}

// ---------------------------------------------------------------------------
// DKG Session management
// ---------------------------------------------------------------------------

pub fn create_dkg_session(session_id: String, dkg: DkgSession) {
    let arc_dkg = Arc::new(Mutex::new(dkg));
    DKG_SESSIONS.lock().unwrap().insert(session_id, arc_dkg);
}

pub fn get_dkg_session_arc(session_id: &str) -> Option<Arc<Mutex<DkgSession>>> {
    DKG_SESSIONS
        .lock()
        .unwrap()
        .get(session_id)
        .map(Arc::clone)
}

pub fn delete_dkg_session(session_id: &str) {
    DKG_SESSIONS.lock().unwrap().remove(session_id);
}

// ---------------------------------------------------------------------------
// Sign Session management
// ---------------------------------------------------------------------------

pub fn create_sign_session(session_id: String, session: SignSession) {
    let arc_session = Arc::new(Mutex::new(session));
    SIGN_SESSIONS.lock().unwrap().insert(session_id, arc_session);
}

pub fn get_sign_session_arc(session_id: &str) -> Option<Arc<Mutex<SignSession>>> {
    SIGN_SESSIONS
        .lock()
        .unwrap()
        .get(session_id)
        .map(Arc::clone)
}

pub fn delete_sign_session(session_id: &str) {
    SIGN_SESSIONS.lock().unwrap().remove(session_id);
}

// ---------------------------------------------------------------------------
// Presign Session management
// ---------------------------------------------------------------------------

pub fn create_presign_session(session_id: String, session: PresignSession) {
    let arc_session = Arc::new(Mutex::new(session));
    PRESIGN_SESSIONS.lock().unwrap().insert(session_id, arc_session);
}

pub fn get_presign_session_arc(session_id: &str) -> Option<Arc<Mutex<PresignSession>>> {
    PRESIGN_SESSIONS
        .lock()
        .unwrap()
        .get(session_id)
        .map(Arc::clone)
}

pub fn delete_presign_session(session_id: &str) {
    PRESIGN_SESSIONS.lock().unwrap().remove(session_id);
}

// ---------------------------------------------------------------------------
// Reshare Session management
// ---------------------------------------------------------------------------

pub fn create_reshare_session(session_id: String, session: ReshareSession) {
    let arc_session = Arc::new(Mutex::new(session));
    RESHARE_SESSIONS.lock().unwrap().insert(session_id, arc_session);
}

pub fn get_reshare_session_arc(session_id: &str) -> Option<Arc<Mutex<ReshareSession>>> {
    RESHARE_SESSIONS
        .lock()
        .unwrap()
        .get(session_id)
        .map(Arc::clone)
}

pub fn delete_reshare_session(session_id: &str) {
    RESHARE_SESSIONS.lock().unwrap().remove(session_id);
}

// ---------------------------------------------------------------------------
// Noise Session management
// ---------------------------------------------------------------------------

pub fn create_noise_session(session_id: String, session: NoiseSession) {
    let arc_session = Arc::new(Mutex::new(session));
    NOISE_SESSIONS.lock().unwrap().insert(session_id, arc_session);
}

pub fn get_noise_session_arc(session_id: &str) -> Option<Arc<Mutex<NoiseSession>>> {
    NOISE_SESSIONS
        .lock()
        .unwrap()
        .get(session_id)
        .map(Arc::clone)
}

pub fn delete_noise_session(session_id: &str) {
    NOISE_SESSIONS.lock().unwrap().remove(session_id);
}

// ---------------------------------------------------------------------------
// Recovery State management
// ---------------------------------------------------------------------------

static RECOVERY_BACKUP_SHARD: Mutex<Option<KeyShare>> = Mutex::new(None);

pub fn store_recovery_backup_shard(shard: KeyShare) {
    *RECOVERY_BACKUP_SHARD.lock().unwrap() = Some(shard);
}

pub fn get_recovery_backup_shard() -> Option<KeyShare> {
    RECOVERY_BACKUP_SHARD.lock().unwrap().clone()
}

pub fn clear_recovery_backup_shard() {
    *RECOVERY_BACKUP_SHARD.lock().unwrap() = None;
}
