use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    pub version: u8,
    pub payload: Vec<u8>,
    pub sender_address: String,
}

impl Envelope {
    pub const PROTOCOL_VERSION: u8 = 1;

    pub fn new(payload: Vec<u8>, sender_address: String) -> Self {
        Self {
            version: Self::PROTOCOL_VERSION,
            payload,
            sender_address,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap_or_default()
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        bincode::deserialize(bytes).ok()
    }
}
