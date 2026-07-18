use anyhow::Result;
use sled::Db;
use std::path::Path;

use crate::crypto::identity::Identity;
use crate::protocol::message::Message;

pub struct Storage {
    db: Db,
}

impl Storage {
    pub fn open(path: &Path) -> Result<Self> {
        let db = sled::open(path)?;
        Ok(Self { db })
    }

    pub fn save_identity(&self, identity: &Identity) -> Result<()> {
        self.db.insert("identity", identity.secret_bytes().as_slice())?;
        self.db.flush()?;
        Ok(())
    }

    pub fn load_identity(&self) -> Result<Option<Identity>> {
        match self.db.get("identity")? {
            Some(bytes) => {
                let secret: [u8; 32] = bytes.as_ref().try_into()
                    .map_err(|_| anyhow::anyhow!("corrupt identity"))?;
                Ok(Some(Identity::from_bytes(&secret)))
            }
            None => Ok(None),
        }
    }

    pub fn save_message(&self, msg: &Message) -> Result<()> {
        let key = format!("msg:{}", msg.id);
        let value = serde_json::to_vec(msg)?;
        self.db.insert(key.as_bytes(), value)?;
        Ok(())
    }

    pub fn get_timeline(&self, limit: usize) -> Result<Vec<Message>> {
        let mut messages = Vec::new();
        for entry in self.db.scan_prefix(b"msg:") {
            let (_, value) = entry?;
            if let Ok(msg) = serde_json::from_slice::<Message>(&value) {
                messages.push(msg);
            }
            if messages.len() >= limit {
                break;
            }
        }
        messages.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(messages)
    }
}
