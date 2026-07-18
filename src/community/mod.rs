use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Community {
    pub id: String,
    pub name: String,
    pub description: String,
    pub owner: String,
    pub members: HashSet<String>,
    pub is_locked: bool,
    pub invite_only: bool,
}

impl Community {
    pub fn new(name: String, description: String, owner: String, locked: bool) -> Self {
        let id = format!(
            "comm:{}",
            &sha2::Sha256::digest(format!("{}:{}", owner, name).as_bytes())
                .iter()
                .take(8)
                .map(|b| format!("{:02x}", b))
                .collect::<String>()
        );

        let mut members = HashSet::new();
        members.insert(owner.clone());

        Self {
            id,
            name,
            description,
            owner,
            members,
            is_locked: locked,
            invite_only: locked,
        }
    }

    pub fn add_member(&mut self, address: &str) -> bool {
        if self.is_locked && !self.members.contains(address) {
            return false;
        }
        self.members.insert(address.to_string())
    }

    pub fn remove_member(&mut self, address: &str, requester: &str) -> bool {
        if requester != self.owner {
            return false;
        }
        self.members.remove(address)
    }

    pub fn is_member(&self, address: &str) -> bool {
        self.members.contains(address)
    }
}

use sha2::Digest;
