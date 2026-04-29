use std::collections::HashMap;
use std::sync::Mutex;

use mpc_core::dkls23::{KeyShare, PartyIndex};

static SHARDS: Mutex<Option<HashMap<PartyIndex, KeyShare>>> = Mutex::new(None);

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
