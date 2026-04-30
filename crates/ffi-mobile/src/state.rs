use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use mpc_core::dkls23::dkg::DkgSession;
use mpc_core::dkls23::{KeyShare, PartyIndex};

static SHARDS: Mutex<Option<HashMap<PartyIndex, KeyShare>>> = Mutex::new(None);
static DKG_SESSIONS: std::sync::LazyLock<Mutex<HashMap<String, Arc<Mutex<DkgSession>>>>> =
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
