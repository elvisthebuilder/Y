use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub author: String,
    pub content: MessageContent,
    pub timestamp: DateTime<Utc>,
    pub signature: Vec<u8>,
    pub reply_to: Option<String>,
    pub nods: Vec<Nod>,
    pub replies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Nod {
    pub from: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageContent {
    Post(PostMessage),
    DirectMessage(DirectMessage),
    CommunityMessage(CommunityMsg),
    Reply(ReplyMessage),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostMessage {
    pub text: String,
    pub media: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplyMessage {
    pub parent_id: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectMessage {
    pub recipient: String,
    pub encrypted_payload: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunityMsg {
    pub community_id: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PeerCommand {
    Announce(AnnouncePayload),
    RequestPosts { since: DateTime<Utc>, limit: u32 },
    DeliverMessages(Vec<Message>),
    NodPost { post_id: String, from: String },
    JoinCommunity { community_id: String, invite_token: Option<String> },
    Ping,
    Pong,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnouncePayload {
    pub address: String,
    pub verifying_key: [u8; 32],
    pub onion_address: String,
    pub timestamp: DateTime<Utc>,
    pub signature: Vec<u8>,
}

impl Message {
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(self.id.as_bytes());
        data.extend_from_slice(self.author.as_bytes());
        data.extend_from_slice(&bincode::serialize(&self.content).unwrap_or_default());
        data.extend_from_slice(self.timestamp.to_rfc3339().as_bytes());
        data
    }

    pub fn nod_count(&self) -> usize {
        self.nods.len()
    }

    pub fn reply_count(&self) -> usize {
        self.replies.len()
    }

    pub fn has_nodded(&self, user: &str) -> bool {
        self.nods.iter().any(|n| n.from == user)
    }
}
