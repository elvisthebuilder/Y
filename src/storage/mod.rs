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
        self.db
            .insert("identity", identity.secret_bytes().as_slice())?;
        self.db.flush()?;
        Ok(())
    }

    pub fn load_identity(&self) -> Result<Option<Identity>> {
        match self.db.get("identity")? {
            Some(bytes) => {
                let secret: [u8; 32] = bytes
                    .as_ref()
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("corrupt identity"))?;
                Ok(Some(Identity::from_bytes(&secret)))
            }
            None => Ok(None),
        }
    }

    pub fn save_alias(&self, alias: &str) -> Result<()> {
        self.db.insert("alias", alias.as_bytes())?;
        self.db.flush()?;
        Ok(())
    }

    pub fn load_alias(&self) -> Result<Option<String>> {
        match self.db.get("alias")? {
            Some(bytes) => Ok(Some(String::from_utf8(bytes.to_vec())?)),
            None => Ok(None),
        }
    }

    pub fn save_message(&self, msg: &Message) -> Result<()> {
        let key = format!("msg:{}", msg.id);
        let value = serde_json::to_vec(msg)?;
        self.db.insert(key.as_bytes(), value)?;
        Ok(())
    }

    pub fn update_message(&self, msg: &Message) -> Result<()> {
        self.save_message(msg)
    }

    pub fn delete_message(&self, id: &str) -> Result<()> {
        let key = format!("msg:{}", id);
        self.db.remove(key.as_bytes())?;
        Ok(())
    }

    pub fn get_message(&self, id: &str) -> Result<Option<Message>> {
        let key = format!("msg:{}", id);
        match self.db.get(key.as_bytes())? {
            Some(value) => Ok(serde_json::from_slice(&value)?),
            None => Ok(None),
        }
    }

    pub fn get_timeline(&self, limit: usize) -> Result<Vec<Message>> {
        let mut messages = Vec::new();
        for entry in self.db.scan_prefix(b"msg:") {
            let (_, value) = entry?;
            if let Ok(msg) = serde_json::from_slice::<Message>(&value) {
                messages.push(msg);
            }
        }
        messages.sort_by_key(|m| std::cmp::Reverse(m.timestamp));
        messages.truncate(limit);
        Ok(messages)
    }

    pub fn bookmark_post(&self, post_id: &str) -> Result<()> {
        let key = format!("bookmark:{}", post_id);
        self.db.insert(key.as_bytes(), b"1")?;
        Ok(())
    }

    pub fn unbookmark_post(&self, post_id: &str) -> Result<()> {
        let key = format!("bookmark:{}", post_id);
        self.db.remove(key.as_bytes())?;
        Ok(())
    }

    pub fn is_bookmarked(&self, post_id: &str) -> bool {
        let key = format!("bookmark:{}", post_id);
        self.db.get(key.as_bytes()).ok().flatten().is_some()
    }

    pub fn get_bookmarked_posts(&self) -> Result<Vec<Message>> {
        let mut messages = Vec::new();
        for entry in self.db.scan_prefix(b"bookmark:") {
            let (key, _) = entry?;
            let post_id = String::from_utf8(key[9..].to_vec())?;
            if let Some(msg) = self.get_message(&post_id)? {
                messages.push(msg);
            }
        }
        messages.sort_by_key(|m| std::cmp::Reverse(m.timestamp));
        Ok(messages)
    }

    pub fn prune_timeline(&self, max_posts: usize) -> Result<usize> {
        let mut entries: Vec<(String, chrono::DateTime<chrono::Utc>)> = Vec::new();
        for entry in self.db.scan_prefix(b"msg:") {
            let (_, value) = entry?;
            if let Ok(msg) = serde_json::from_slice::<Message>(&value) {
                entries.push((msg.id, msg.timestamp));
            }
        }
        if entries.len() <= max_posts {
            return Ok(0);
        }
        entries.sort_by_key(|(_, ts)| std::cmp::Reverse(*ts));
        let mut pruned = 0;
        for (id, _) in &entries[max_posts..] {
            let key = format!("msg:{}", id);
            self.db.remove(key.as_bytes())?;
            pruned += 1;
        }
        Ok(pruned)
    }
}
