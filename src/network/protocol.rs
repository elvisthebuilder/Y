use super::dht::{DhtRequest, DhtResponse};
use crate::protocol::message::Message;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WireMessage {
    Hello(HelloPayload),
    HelloAck(HelloPayload),

    // Gossip
    BroadcastPost(Message),
    RequestTimeline {
        since: Option<DateTime<Utc>>,
        limit: u32,
    },
    TimelineResponse(Vec<Message>),

    // DMs
    DirectMessage(EncryptedEnvelope),
    RequestPendingDms {
        recipient: String,
    },
    PendingDmsResponse(Vec<EncryptedEnvelope>),

    // Peer exchange
    RequestPeers,
    PeersResponse(Vec<PeerAnnounce>),

    // Nods propagation
    NodNotify {
        post_id: String,
        from: String,
    },
    NodRemove {
        post_id: String,
        from: String,
    },

    // DHT operations
    DhtRequest {
        request_id: u64,
        request: DhtRequest,
    },
    DhtResponse {
        request_id: u64,
        response: DhtResponse,
    },

    // Keep-alive
    Ping(u64),
    Pong(u64),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloPayload {
    pub address: String,
    pub alias: String,
    pub verifying_key: [u8; 32],
    pub listen_addr: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedEnvelope {
    pub recipient: String,
    pub sender: String,
    pub ephemeral_public: [u8; 32],
    pub nonce: [u8; 12],
    pub ciphertext: Vec<u8>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerAnnounce {
    pub address: String,
    pub alias: String,
    pub listen_addr: String,
    pub last_seen: DateTime<Utc>,
}
