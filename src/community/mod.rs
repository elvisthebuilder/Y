use serde::{Deserialize, Serialize};
use sha2::Digest;
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
    pub pending_requests: Vec<String>,
}

impl Community {
    pub fn new(name: String, description: String, owner: String, locked: bool) -> Self {
        let id = format!(
            "comm:{}",
            sha2::Sha256::digest(format!("{}:{}", owner, name).as_bytes())
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
            pending_requests: Vec::new(),
        }
    }

    pub fn request_join(&mut self, address: &str) -> JoinResult {
        if self.members.contains(address) {
            return JoinResult::AlreadyMember;
        }
        if self.is_locked {
            if self.pending_requests.contains(&address.to_string()) {
                JoinResult::AlreadyPending
            } else {
                self.pending_requests.push(address.to_string());
                JoinResult::Pending
            }
        } else {
            self.members.insert(address.to_string());
            JoinResult::Joined
        }
    }

    pub fn approve_request(&mut self, address: &str, requester: &str) -> bool {
        if requester != self.owner {
            return false;
        }
        if let Some(pos) = self.pending_requests.iter().position(|a| a == address) {
            self.pending_requests.remove(pos);
            self.members.insert(address.to_string());
            true
        } else {
            false
        }
    }

    pub fn decline_request(&mut self, address: &str, requester: &str) -> bool {
        if requester != self.owner {
            return false;
        }
        if let Some(pos) = self.pending_requests.iter().position(|a| a == address) {
            self.pending_requests.remove(pos);
            true
        } else {
            false
        }
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

#[derive(Debug, PartialEq)]
pub enum JoinResult {
    Joined,
    Pending,
    AlreadyMember,
    AlreadyPending,
}
