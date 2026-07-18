use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::crypto::identity::PublicIdentity;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub identity: PublicIdentity,
    pub onion_address: String,
    pub last_seen: DateTime<Utc>,
    pub reputation: i32,
}

pub struct PeerRegistry {
    peers: Arc<RwLock<HashMap<String, PeerInfo>>>,
}

impl PeerRegistry {
    pub fn new() -> Self {
        Self {
            peers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn register_peer(&self, info: PeerInfo) {
        let mut peers = self.peers.write().await;
        peers.insert(info.identity.address.clone(), info);
    }

    pub async fn remove_peer(&self, address: &str) {
        let mut peers = self.peers.write().await;
        peers.remove(address);
    }

    pub async fn get_peer(&self, address: &str) -> Option<PeerInfo> {
        let peers = self.peers.read().await;
        peers.get(address).cloned()
    }

    pub async fn all_peers(&self) -> Vec<PeerInfo> {
        let peers = self.peers.read().await;
        peers.values().cloned().collect()
    }

    pub async fn peer_count(&self) -> usize {
        let peers = self.peers.read().await;
        peers.len()
    }
}
