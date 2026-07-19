use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

const K: usize = 20; // max peers per bucket
const ALPHA: usize = 3; // parallel lookups
const REPLICATION: usize = 5; // store on this many closest nodes
const KEY_BITS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub [u8; 32]);

impl NodeId {
    pub fn from_address(address: &str) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(address.as_bytes());
        let result = hasher.finalize();
        let mut id = [0u8; 32];
        id.copy_from_slice(&result);
        Self(id)
    }

    pub fn from_key(key: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(key);
        let result = hasher.finalize();
        let mut id = [0u8; 32];
        id.copy_from_slice(&result);
        Self(id)
    }

    pub fn distance(&self, other: &NodeId) -> [u8; 32] {
        let mut dist = [0u8; 32];
        for (i, byte) in dist.iter_mut().enumerate() {
            *byte = self.0[i] ^ other.0[i];
        }
        dist
    }

    fn leading_zeros(distance: &[u8; 32]) -> usize {
        for (i, byte) in distance.iter().enumerate() {
            if *byte != 0 {
                return i * 8 + byte.leading_zeros() as usize;
            }
        }
        KEY_BITS
    }

    pub fn bucket_index(&self, other: &NodeId) -> usize {
        let dist = self.distance(other);
        let lz = Self::leading_zeros(&dist);
        if lz >= KEY_BITS {
            KEY_BITS - 1
        } else {
            KEY_BITS - 1 - lz
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DhtNode {
    pub id: NodeId,
    pub address: String,
    pub onion_addr: String,
    pub last_seen: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DhtValue {
    Post(StoredPost),
    DirectMessage(StoredDm),
    CommunityPost(StoredCommunityPost),
    PeerAnnounce(StoredPeerAnnounce),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredPost {
    pub id: String,
    pub author: String,
    pub content: Vec<u8>,
    pub signature: Vec<u8>,
    pub timestamp: DateTime<Utc>,
    pub ttl: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredDm {
    pub recipient_hash: [u8; 32],
    pub sender: String,
    pub ephemeral_public: [u8; 32],
    pub nonce: [u8; 12],
    pub ciphertext: Vec<u8>,
    pub timestamp: DateTime<Utc>,
    pub ttl: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCommunityPost {
    pub community_id: String,
    pub post_id: String,
    pub encrypted_content: Vec<u8>,
    pub author: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredPeerAnnounce {
    pub address: String,
    pub alias: String,
    pub onion_addr: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DhtRequest {
    Ping,
    FindNode { target: NodeId },
    FindValue { key: NodeId },
    Store { key: NodeId, value: DhtValue },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DhtResponse {
    Pong,
    NodesFound(Vec<DhtNode>),
    ValueFound(Vec<DhtValue>),
    Stored,
    NotFound,
}

pub struct RoutingTable {
    local_id: NodeId,
    buckets: Vec<Vec<DhtNode>>,
}

impl RoutingTable {
    pub fn new(local_id: NodeId) -> Self {
        let buckets = (0..KEY_BITS).map(|_| Vec::new()).collect();
        Self { local_id, buckets }
    }

    pub fn insert(&mut self, node: DhtNode) {
        if node.id == self.local_id {
            return;
        }
        let idx = self.local_id.bucket_index(&node.id);
        let bucket = &mut self.buckets[idx];

        if let Some(pos) = bucket.iter().position(|n| n.id == node.id) {
            bucket.remove(pos);
            bucket.push(node);
        } else if bucket.len() < K {
            bucket.push(node);
        } else {
            // Bucket full — evict oldest if it's stale
            if bucket[0].last_seen < Utc::now() - chrono::Duration::hours(1) {
                bucket.remove(0);
                bucket.push(node);
            }
        }
    }

    pub fn closest_nodes(&self, target: &NodeId, count: usize) -> Vec<DhtNode> {
        let mut all_nodes: Vec<&DhtNode> = self.buckets.iter().flatten().collect();
        all_nodes.sort_by(|a, b| {
            let da = target.distance(&a.id);
            let db = target.distance(&b.id);
            da.cmp(&db)
        });
        all_nodes.into_iter().take(count).cloned().collect()
    }

    pub fn node_count(&self) -> usize {
        self.buckets.iter().map(|b| b.len()).sum()
    }

    pub fn all_nodes(&self) -> Vec<DhtNode> {
        self.buckets.iter().flatten().cloned().collect()
    }
}

pub struct DhtStorage {
    data: HashMap<[u8; 32], Vec<DhtValue>>,
    max_entries: usize,
}

impl DhtStorage {
    pub fn new(max_entries: usize) -> Self {
        Self {
            data: HashMap::new(),
            max_entries,
        }
    }

    pub fn store(&mut self, key: &NodeId, value: DhtValue) {
        let entry = self.data.entry(key.0).or_default();
        entry.push(value);

        // Evict oldest entries if over capacity
        if self.total_entries() > self.max_entries {
            self.evict_oldest();
        }
    }

    pub fn get(&self, key: &NodeId) -> Option<&Vec<DhtValue>> {
        self.data.get(&key.0)
    }

    pub fn get_for_recipient(&self, recipient_hash: &[u8; 32]) -> Vec<DhtValue> {
        self.data
            .values()
            .flatten()
            .filter(|v| match v {
                DhtValue::DirectMessage(dm) => &dm.recipient_hash == recipient_hash,
                _ => false,
            })
            .cloned()
            .collect()
    }

    pub fn remove_delivered_dms(&mut self, recipient_hash: &[u8; 32]) {
        for values in self.data.values_mut() {
            values.retain(|v| match v {
                DhtValue::DirectMessage(dm) => &dm.recipient_hash != recipient_hash,
                _ => true,
            });
        }
        self.data.retain(|_, v| !v.is_empty());
    }

    fn total_entries(&self) -> usize {
        self.data.values().map(|v| v.len()).sum()
    }

    fn evict_oldest(&mut self) {
        // Remove entries with oldest timestamps
        let mut all_keys: Vec<[u8; 32]> = self.data.keys().cloned().collect();
        all_keys.sort_by(|a, b| {
            let ts_a = self.oldest_timestamp(a);
            let ts_b = self.oldest_timestamp(b);
            ts_a.cmp(&ts_b)
        });

        while self.total_entries() > self.max_entries {
            if let Some(key) = all_keys.first() {
                self.data.remove(key);
                all_keys.remove(0);
            } else {
                break;
            }
        }
    }

    fn oldest_timestamp(&self, key: &[u8; 32]) -> DateTime<Utc> {
        self.data
            .get(key)
            .and_then(|values| values.first())
            .map(|v| match v {
                DhtValue::Post(p) => p.timestamp,
                DhtValue::DirectMessage(dm) => dm.timestamp,
                DhtValue::CommunityPost(cp) => cp.timestamp,
                DhtValue::PeerAnnounce(pa) => pa.timestamp,
            })
            .unwrap_or_else(Utc::now)
    }
}

pub struct Dht {
    pub local_id: NodeId,
    pub routing_table: Arc<RwLock<RoutingTable>>,
    pub storage: Arc<RwLock<DhtStorage>>,
}

impl Dht {
    pub fn new(address: &str) -> Self {
        let local_id = NodeId::from_address(address);
        Self {
            local_id: local_id.clone(),
            routing_table: Arc::new(RwLock::new(RoutingTable::new(local_id))),
            storage: Arc::new(RwLock::new(DhtStorage::new(10_000))),
        }
    }

    pub async fn add_node(&self, node: DhtNode) {
        let mut rt = self.routing_table.write().await;
        rt.insert(node);
    }

    pub async fn find_closest(&self, target: &NodeId) -> Vec<DhtNode> {
        let rt = self.routing_table.read().await;
        rt.closest_nodes(target, REPLICATION)
    }

    pub async fn store_value(&self, key: &NodeId, value: DhtValue) {
        let mut storage = self.storage.write().await;
        storage.store(key, value);
    }

    pub async fn get_value(&self, key: &NodeId) -> Option<Vec<DhtValue>> {
        let storage = self.storage.read().await;
        storage.get(key).cloned()
    }

    pub fn post_key(post_id: &str) -> NodeId {
        NodeId::from_key(post_id.as_bytes())
    }

    pub fn timeline_key(author_address: &str) -> NodeId {
        let mut hasher = Sha256::new();
        hasher.update(b"timeline:");
        hasher.update(author_address.as_bytes());
        let result = hasher.finalize();
        let mut id = [0u8; 32];
        id.copy_from_slice(&result);
        NodeId(id)
    }

    pub fn dm_key(recipient_address: &str) -> NodeId {
        let mut hasher = Sha256::new();
        hasher.update(b"dm:");
        hasher.update(recipient_address.as_bytes());
        let result = hasher.finalize();
        let mut id = [0u8; 32];
        id.copy_from_slice(&result);
        NodeId(id)
    }

    pub fn peer_registry_key() -> NodeId {
        NodeId::from_key(b"y:peer-registry:global")
    }

    pub fn community_key(community_id: &str) -> NodeId {
        let mut hasher = Sha256::new();
        hasher.update(b"community:");
        hasher.update(community_id.as_bytes());
        let result = hasher.finalize();
        let mut id = [0u8; 32];
        id.copy_from_slice(&result);
        NodeId(id)
    }

    pub async fn handle_request(&self, request: DhtRequest) -> DhtResponse {
        match request {
            DhtRequest::Ping => DhtResponse::Pong,
            DhtRequest::FindNode { target } => {
                let rt = self.routing_table.read().await;
                let nodes = rt.closest_nodes(&target, K);
                DhtResponse::NodesFound(nodes)
            }
            DhtRequest::FindValue { key } => {
                let storage = self.storage.read().await;
                match storage.get(&key) {
                    Some(values) => DhtResponse::ValueFound(values.clone()),
                    None => {
                        drop(storage);
                        let rt = self.routing_table.read().await;
                        let nodes = rt.closest_nodes(&key, K);
                        DhtResponse::NodesFound(nodes)
                    }
                }
            }
            DhtRequest::Store { key, value } => {
                let mut storage = self.storage.write().await;
                storage.store(&key, value);
                DhtResponse::Stored
            }
        }
    }

    pub async fn retrieve_dms(&self, recipient_address: &str) -> Vec<DhtValue> {
        let mut hasher = Sha256::new();
        hasher.update(recipient_address.as_bytes());
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);

        let storage = self.storage.read().await;
        storage.get_for_recipient(&hash)
    }

    pub async fn clear_delivered_dms(&self, recipient_address: &str) {
        let mut hasher = Sha256::new();
        hasher.update(recipient_address.as_bytes());
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);

        let mut storage = self.storage.write().await;
        storage.remove_delivered_dms(&hash);
    }

    pub async fn node_count(&self) -> usize {
        let rt = self.routing_table.read().await;
        rt.node_count()
    }
}
