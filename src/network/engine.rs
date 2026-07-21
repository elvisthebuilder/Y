use anyhow::Result;
use arti_client::DataStream;
use chrono::Utc;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{info, warn};

use super::codec::FramedStream;
use super::dht::{Dht, DhtNode, DhtValue, NodeId, StoredDm, StoredPeerAnnounce, StoredPost};
use super::protocol::{EncryptedEnvelope, HelloPayload, PeerAnnounce, WireMessage};
use super::tor::TorTransport;
use crate::crypto::identity::Identity;
use crate::protocol::message::Message;

// The Mediator — seed node for initial peer discovery.
const SEED_NODES: &[&str] =
    &["kfpa2iyzmurhkpaqn4kf2ez5ztojplhbq5z2jzfv3jdzhuspqx4fliad.onion:7331"];

#[derive(Debug, Clone)]
pub enum NetworkEvent {
    PeerConnected { address: String, alias: String },
    PeerDisconnected { address: String },
    NewPost(Message),
    NewDirectMessage(EncryptedEnvelope),
    NodReceived { post_id: String, from: String },
    NodRemoved { post_id: String, from: String },
    PeerCountChanged(usize),
    OnionReady(String),
    ConnectivityChanged(bool),
}

pub struct NetworkEngine {
    identity: Identity,
    alias: String,
    listen_port: u16,
    data_dir: PathBuf,
    peers: Arc<RwLock<HashMap<String, PeerConnection>>>,
    event_tx: mpsc::UnboundedSender<NetworkEvent>,
    known_posts: Arc<RwLock<Vec<String>>>,
    known_nod_events: Arc<RwLock<Vec<String>>>,
    tor: Arc<RwLock<Option<TorTransport>>>,
    pub dht: Arc<Dht>,
}

struct PeerConnection {
    address: String,
    alias: String,
    onion_addr: String,
    verifying_key: [u8; 32],
}

impl NetworkEngine {
    pub fn new(
        identity: Identity,
        alias: String,
        listen_port: u16,
        data_dir: PathBuf,
        event_tx: mpsc::UnboundedSender<NetworkEvent>,
    ) -> Self {
        let dht = Arc::new(Dht::new(&identity.address));
        Self {
            identity,
            alias,
            listen_port,
            data_dir,
            peers: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
            known_posts: Arc::new(RwLock::new(Vec::new())),
            known_nod_events: Arc::new(RwLock::new(Vec::new())),
            tor: Arc::new(RwLock::new(None)),
            dht,
        }
    }

    pub async fn start(self: Arc<Self>) -> Result<()> {
        let mut transport = TorTransport::bootstrap(&self.data_dir).await?;
        let mut incoming = transport.start_hidden_service(self.listen_port).await?;

        if let Some(addr) = transport.onion_address() {
            let _ = self
                .event_tx
                .send(NetworkEvent::OnionReady(addr.to_string()));
            info!("Y node reachable at: {}", addr);
        }

        {
            let mut tor_lock = self.tor.write().await;
            *tor_lock = Some(transport);
        }

        info!("Network engine listening via Tor hidden service");

        // On startup, check DHT for pending DMs addressed to us
        let engine_dm = Arc::clone(&self);
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            engine_dm.fetch_pending_dms().await;
        });

        while let Some(stream) = incoming.recv().await {
            let engine = Arc::clone(&self);
            tokio::spawn(async move {
                if let Err(e) = engine.handle_incoming(stream).await {
                    warn!("Connection error: {}", e);
                }
            });
        }

        Ok(())
    }

    pub async fn connect_to(&self, onion_addr: &str) -> Result<()> {
        info!("Connecting to peer at {}", onion_addr);
        let tor_lock = self.tor.read().await;
        let tor = tor_lock
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Tor not bootstrapped yet"))?;
        let stream = tor.connect(onion_addr).await?;
        drop(tor_lock);
        self.perform_handshake(stream, true).await
    }

    async fn handle_incoming(&self, stream: DataStream) -> Result<()> {
        self.perform_handshake(stream, false).await
    }

    async fn perform_handshake(&self, stream: DataStream, initiator: bool) -> Result<()> {
        let mut framed = FramedStream::new(stream);

        let my_onion = {
            let tor_lock = self.tor.read().await;
            tor_lock
                .as_ref()
                .and_then(|t| t.onion_address().map(|s| s.to_string()))
                .unwrap_or_default()
        };

        let my_hello = HelloPayload {
            address: self.identity.address.clone(),
            alias: self.alias.clone(),
            verifying_key: self.identity.verifying_key.to_bytes(),
            listen_addr: my_onion,
            timestamp: Utc::now(),
        };

        if initiator {
            framed.send_json(&WireMessage::Hello(my_hello)).await?;
            let response: WireMessage = framed.recv_json().await?;
            match response {
                WireMessage::HelloAck(peer_hello) => {
                    self.register_peer(&peer_hello).await;
                }
                _ => return Err(anyhow::anyhow!("unexpected handshake response")),
            }
        } else {
            let msg: WireMessage = framed.recv_json().await?;
            match msg {
                WireMessage::Hello(peer_hello) => {
                    self.register_peer(&peer_hello).await;
                    framed.send_json(&WireMessage::HelloAck(my_hello)).await?;
                }
                _ => return Err(anyhow::anyhow!("expected Hello")),
            }
        }

        self.handle_peer_session(framed).await
    }

    async fn register_peer(&self, hello: &HelloPayload) {
        let mut peers = self.peers.write().await;
        peers.insert(
            hello.address.clone(),
            PeerConnection {
                address: hello.address.clone(),
                alias: hello.alias.clone(),
                onion_addr: hello.listen_addr.clone(),
                verifying_key: hello.verifying_key,
            },
        );
        let count = peers.len();
        drop(peers);

        // Add to DHT routing table
        self.dht
            .add_node(DhtNode {
                id: NodeId::from_address(&hello.address),
                address: hello.address.clone(),
                onion_addr: hello.listen_addr.clone(),
                last_seen: Utc::now(),
            })
            .await;

        let _ = self.event_tx.send(NetworkEvent::PeerConnected {
            address: hello.address.clone(),
            alias: hello.alias.clone(),
        });
        let _ = self.event_tx.send(NetworkEvent::PeerCountChanged(count));
        info!("Peer registered: {} ({})", hello.alias, hello.address);
    }

    async fn handle_peer_session(&self, mut framed: FramedStream<DataStream>) -> Result<()> {
        loop {
            let msg: WireMessage = match framed.recv_json().await {
                Ok(msg) => msg,
                Err(_) => break,
            };

            match msg {
                WireMessage::BroadcastPost(post) => {
                    let mut known = self.known_posts.write().await;
                    if !known.contains(&post.id) {
                        known.push(post.id.clone());
                        drop(known);
                        let _ = self.event_tx.send(NetworkEvent::NewPost(post.clone()));

                        // Store in DHT for persistence
                        self.dht_store_post(&post).await;

                        // Relay to other connected peers
                        self.relay_post(&post).await;
                    }
                }
                WireMessage::RequestTimeline { since, limit } => {
                    let timeline = self.dht_get_timeline(since, limit).await;
                    framed
                        .send_json(&WireMessage::TimelineResponse(timeline))
                        .await?;
                }
                WireMessage::DirectMessage(envelope) => {
                    if envelope.recipient == self.identity.address {
                        let _ = self.event_tx.send(NetworkEvent::NewDirectMessage(envelope));
                    } else {
                        // Store in DHT for the recipient to retrieve later
                        self.dht_store_dm(&envelope).await;
                    }
                }
                WireMessage::RequestPendingDms { recipient } => {
                    let dms = self.dht.retrieve_dms(&recipient).await;
                    let envelopes: Vec<EncryptedEnvelope> = dms
                        .into_iter()
                        .filter_map(|v| {
                            if let DhtValue::DirectMessage(stored) = v {
                                Some(EncryptedEnvelope {
                                    recipient: recipient.clone(),
                                    sender: stored.sender,
                                    ephemeral_public: stored.ephemeral_public,
                                    nonce: stored.nonce,
                                    ciphertext: stored.ciphertext,
                                    timestamp: stored.timestamp,
                                })
                            } else {
                                None
                            }
                        })
                        .collect();
                    framed
                        .send_json(&WireMessage::PendingDmsResponse(envelopes))
                        .await?;
                    self.dht.clear_delivered_dms(&recipient).await;
                }
                WireMessage::PendingDmsResponse(_) => {}
                WireMessage::NodNotify {
                    ref post_id,
                    ref from,
                } => {
                    let nod_key = format!("nod:{}:{}", post_id, from);
                    let mut known = self.known_nod_events.write().await;
                    if !known.contains(&nod_key) {
                        known.push(nod_key);
                        drop(known);
                        let _ = self.event_tx.send(NetworkEvent::NodReceived {
                            post_id: post_id.clone(),
                            from: from.clone(),
                        });
                        self.relay_nod_event(&msg, from).await;
                    }
                }
                WireMessage::NodRemove {
                    ref post_id,
                    ref from,
                } => {
                    let nod_key = format!("unnod:{}:{}", post_id, from);
                    let mut known = self.known_nod_events.write().await;
                    if !known.contains(&nod_key) {
                        known.push(nod_key);
                        drop(known);
                        let _ = self.event_tx.send(NetworkEvent::NodRemoved {
                            post_id: post_id.clone(),
                            from: from.clone(),
                        });
                        self.relay_nod_event(&msg, from).await;
                    }
                }
                WireMessage::RequestPeers => {
                    let peers = self.peers.read().await;
                    let announces: Vec<PeerAnnounce> = peers
                        .values()
                        .map(|p| PeerAnnounce {
                            address: p.address.clone(),
                            alias: p.alias.clone(),
                            listen_addr: p.onion_addr.clone(),
                            last_seen: Utc::now(),
                        })
                        .collect();
                    framed
                        .send_json(&WireMessage::PeersResponse(announces))
                        .await?;
                }
                WireMessage::PeersResponse(new_peers) => {
                    for peer in new_peers {
                        let existing = self.peers.read().await;
                        if !existing.contains_key(&peer.address)
                            && peer.address != self.identity.address
                        {
                            drop(existing);
                            info!(
                                "Discovered new peer: {} at {}",
                                peer.alias, peer.listen_addr
                            );

                            // Add to DHT routing table even if we can't connect yet
                            self.dht
                                .add_node(DhtNode {
                                    id: NodeId::from_address(&peer.address),
                                    address: peer.address.clone(),
                                    onion_addr: peer.listen_addr.clone(),
                                    last_seen: Utc::now(),
                                })
                                .await;

                            let tor_lock = self.tor.read().await;
                            if let Some(tor) = tor_lock.as_ref() {
                                if let Ok(stream) = tor.connect(&peer.listen_addr).await {
                                    drop(tor_lock);
                                    let mut new_framed = FramedStream::new(stream);
                                    let my_onion = {
                                        let tl = self.tor.read().await;
                                        tl.as_ref()
                                            .and_then(|t| t.onion_address().map(|s| s.to_string()))
                                            .unwrap_or_default()
                                    };
                                    let hello = HelloPayload {
                                        address: self.identity.address.clone(),
                                        alias: self.alias.clone(),
                                        verifying_key: self.identity.verifying_key.to_bytes(),
                                        listen_addr: my_onion,
                                        timestamp: Utc::now(),
                                    };
                                    let _ = new_framed.send_json(&WireMessage::Hello(hello)).await;
                                    if let Ok(WireMessage::HelloAck(ack)) =
                                        new_framed.recv_json().await
                                    {
                                        self.register_peer(&ack).await;
                                    }
                                }
                            }
                        }
                    }
                }
                WireMessage::DhtRequest {
                    request_id,
                    request,
                } => {
                    let response = self.dht.handle_request(request).await;
                    framed
                        .send_json(&WireMessage::DhtResponse {
                            request_id,
                            response,
                        })
                        .await?;
                }
                WireMessage::DhtResponse { .. } => {
                    // Responses are handled by the requester's task
                }
                WireMessage::Ping(nonce) => {
                    framed.send_json(&WireMessage::Pong(nonce)).await?;
                }
                WireMessage::Pong(_) => {}
                _ => {}
            }
        }

        Ok(())
    }

    async fn dht_store_post(&self, post: &Message) {
        let content = serde_json::to_vec(post).unwrap_or_default();
        let signature = post.signature.clone();

        let stored = StoredPost {
            id: post.id.clone(),
            author: post.author.clone(),
            content,
            signature,
            timestamp: post.timestamp,
            ttl: 86400 * 7, // 7 days
        };

        // Store at the post's key
        let key = Dht::post_key(&post.id);
        self.dht
            .store_value(&key, DhtValue::Post(stored.clone()))
            .await;

        // Also store in author's timeline key
        let timeline_key = Dht::timeline_key(&post.author);
        self.dht
            .store_value(&timeline_key, DhtValue::Post(stored))
            .await;
    }

    async fn dht_store_dm(&self, envelope: &EncryptedEnvelope) {
        let mut hasher = sha2::Sha256::new();
        use sha2::Digest;
        hasher.update(envelope.recipient.as_bytes());
        let result = hasher.finalize();
        let mut recipient_hash = [0u8; 32];
        recipient_hash.copy_from_slice(&result);

        let stored = StoredDm {
            recipient_hash,
            sender: envelope.sender.clone(),
            ephemeral_public: envelope.ephemeral_public,
            nonce: envelope.nonce,
            ciphertext: envelope.ciphertext.clone(),
            timestamp: envelope.timestamp,
            ttl: 86400 * 3, // 3 days
        };

        let key = Dht::dm_key(&envelope.recipient);
        self.dht
            .store_value(&key, DhtValue::DirectMessage(stored))
            .await;
    }

    async fn dht_get_timeline(
        &self,
        _since: Option<chrono::DateTime<Utc>>,
        limit: u32,
    ) -> Vec<Message> {
        let storage = self.dht.storage.read().await;
        let posts = storage.get_all_posts(limit as usize);
        drop(storage);

        posts
            .into_iter()
            .filter_map(|v| {
                if let DhtValue::Post(stored) = v {
                    serde_json::from_slice(&stored.content).ok()
                } else {
                    None
                }
            })
            .collect()
    }

    async fn fetch_pending_dms(&self) {
        // Check local DHT first
        let dms = self.dht.retrieve_dms(&self.identity.address).await;
        for dm in dms {
            if let DhtValue::DirectMessage(stored) = dm {
                let envelope = EncryptedEnvelope {
                    recipient: self.identity.address.clone(),
                    sender: stored.sender,
                    ephemeral_public: stored.ephemeral_public,
                    nonce: stored.nonce,
                    ciphertext: stored.ciphertext,
                    timestamp: stored.timestamp,
                };
                let _ = self.event_tx.send(NetworkEvent::NewDirectMessage(envelope));
            }
        }
        self.dht.clear_delivered_dms(&self.identity.address).await;

        // Also query connected peers (mediator) for pending DMs
        let peers: Vec<String> = {
            let p = self.peers.read().await;
            p.values().map(|pc| pc.onion_addr.clone()).collect()
        };

        let tor_lock = self.tor.read().await;
        if let Some(tor) = tor_lock.as_ref() {
            let my_onion = tor
                .onion_address()
                .map(|s| s.to_string())
                .unwrap_or_default();
            for onion in peers {
                if let Ok(stream) = tor.connect(&onion).await {
                    let mut framed = FramedStream::new(stream);
                    let hello = HelloPayload {
                        address: self.identity.address.clone(),
                        alias: self.alias.clone(),
                        verifying_key: self.identity.verifying_key.to_bytes(),
                        listen_addr: my_onion.clone(),
                        timestamp: Utc::now(),
                    };
                    let _ = framed.send_json(&WireMessage::Hello(hello)).await;
                    if let Ok(WireMessage::HelloAck(_)) = framed.recv_json().await {
                        let req = WireMessage::RequestPendingDms {
                            recipient: self.identity.address.clone(),
                        };
                        let _ = framed.send_json(&req).await;
                        if let Ok(WireMessage::PendingDmsResponse(envelopes)) =
                            framed.recv_json().await
                        {
                            for envelope in envelopes {
                                let _ =
                                    self.event_tx.send(NetworkEvent::NewDirectMessage(envelope));
                            }
                        }
                    }
                }
            }
        }
    }

    pub async fn broadcast_post(&self, post: &Message) -> Result<()> {
        let mut known = self.known_posts.write().await;
        known.push(post.id.clone());
        drop(known);

        // Store in DHT for persistence
        self.dht_store_post(post).await;

        // Also broadcast to directly connected peers for real-time delivery
        let msg = WireMessage::BroadcastPost(post.clone());

        let peers = self.peers.read().await;
        let tor_lock = self.tor.read().await;
        if let Some(tor) = tor_lock.as_ref() {
            let my_onion = tor
                .onion_address()
                .map(|s| s.to_string())
                .unwrap_or_default();
            for peer in peers.values() {
                if let Ok(stream) = tor.connect(&peer.onion_addr).await {
                    let mut framed = FramedStream::new(stream);
                    let hello = HelloPayload {
                        address: self.identity.address.clone(),
                        alias: self.alias.clone(),
                        verifying_key: self.identity.verifying_key.to_bytes(),
                        listen_addr: my_onion.clone(),
                        timestamp: Utc::now(),
                    };
                    let _ = framed.send_json(&WireMessage::Hello(hello)).await;
                    if let Ok(WireMessage::HelloAck(_)) = framed.recv_json().await {
                        let _ = framed.send_json(&msg).await;
                    }
                }
            }
        }

        // Replicate to closest DHT nodes
        let key = Dht::post_key(&post.id);
        self.dht_replicate(&key, post).await;

        Ok(())
    }

    async fn dht_replicate(&self, key: &NodeId, post: &Message) {
        let closest = self.dht.find_closest(key).await;
        let content = serde_json::to_vec(post).unwrap_or_default();
        let signature = post.signature.clone();

        let stored = StoredPost {
            id: post.id.clone(),
            author: post.author.clone(),
            content,
            signature,
            timestamp: post.timestamp,
            ttl: 86400 * 7,
        };

        let tor_lock = self.tor.read().await;
        if let Some(tor) = tor_lock.as_ref() {
            for node in closest {
                if node.address == self.identity.address {
                    continue;
                }
                if let Ok(stream) = tor.connect(&node.onion_addr).await {
                    let mut framed = FramedStream::new(stream);
                    let store_req = WireMessage::DhtRequest {
                        request_id: rand::random(),
                        request: super::dht::DhtRequest::Store {
                            key: key.clone(),
                            value: DhtValue::Post(stored.clone()),
                        },
                    };
                    let _ = framed.send_json(&store_req).await;
                }
            }
        }
    }

    pub async fn send_dm(&self, envelope: EncryptedEnvelope) -> Result<()> {
        // Store in DHT so recipient can retrieve even if offline
        self.dht_store_dm(&envelope).await;

        let msg = WireMessage::DirectMessage(envelope.clone());

        let peers = self.peers.read().await;
        let tor_lock = self.tor.read().await;
        if let Some(tor) = tor_lock.as_ref() {
            let my_onion = tor
                .onion_address()
                .map(|s| s.to_string())
                .unwrap_or_default();

            // Try direct delivery first
            if let Some(peer) = peers.get(&envelope.recipient) {
                if let Ok(stream) = tor.connect(&peer.onion_addr).await {
                    let mut framed = FramedStream::new(stream);
                    let hello = HelloPayload {
                        address: self.identity.address.clone(),
                        alias: self.alias.clone(),
                        verifying_key: self.identity.verifying_key.to_bytes(),
                        listen_addr: my_onion.clone(),
                        timestamp: Utc::now(),
                    };
                    let _ = framed.send_json(&WireMessage::Hello(hello)).await;
                    if let Ok(WireMessage::HelloAck(_)) = framed.recv_json().await {
                        let _ = framed.send_json(&msg).await;
                        return Ok(());
                    }
                }
            }

            // Replicate DM to DHT nodes closest to recipient's key
            let key = Dht::dm_key(&envelope.recipient);
            let closest = self.dht.find_closest(&key).await;
            for node in closest {
                if node.address == self.identity.address {
                    continue;
                }
                if let Ok(stream) = tor.connect(&node.onion_addr).await {
                    let mut framed = FramedStream::new(stream);
                    let hello = HelloPayload {
                        address: self.identity.address.clone(),
                        alias: self.alias.clone(),
                        verifying_key: self.identity.verifying_key.to_bytes(),
                        listen_addr: my_onion.clone(),
                        timestamp: Utc::now(),
                    };
                    let _ = framed.send_json(&WireMessage::Hello(hello)).await;
                    if let Ok(WireMessage::HelloAck(_)) = framed.recv_json().await {
                        let _ = framed.send_json(&msg).await;
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn broadcast_nod(&self, post_id: &str) -> Result<()> {
        let nod_key = format!("nod:{}:{}", post_id, self.identity.address);
        self.known_nod_events.write().await.push(nod_key);

        let msg = WireMessage::NodNotify {
            post_id: post_id.to_string(),
            from: self.identity.address.clone(),
        };

        let peers = self.peers.read().await;
        let tor_lock = self.tor.read().await;
        if let Some(tor) = tor_lock.as_ref() {
            let my_onion = tor
                .onion_address()
                .map(|s| s.to_string())
                .unwrap_or_default();
            for peer in peers.values() {
                if let Ok(stream) = tor.connect(&peer.onion_addr).await {
                    let mut framed = FramedStream::new(stream);
                    let hello = HelloPayload {
                        address: self.identity.address.clone(),
                        alias: self.alias.clone(),
                        verifying_key: self.identity.verifying_key.to_bytes(),
                        listen_addr: my_onion.clone(),
                        timestamp: Utc::now(),
                    };
                    let _ = framed.send_json(&WireMessage::Hello(hello)).await;
                    if let Ok(WireMessage::HelloAck(_)) = framed.recv_json().await {
                        let _ = framed.send_json(&msg).await;
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn broadcast_unnod(&self, post_id: &str) -> Result<()> {
        let nod_key = format!("unnod:{}:{}", post_id, self.identity.address);
        self.known_nod_events.write().await.push(nod_key);

        let msg = WireMessage::NodRemove {
            post_id: post_id.to_string(),
            from: self.identity.address.clone(),
        };

        let peers = self.peers.read().await;
        let tor_lock = self.tor.read().await;
        if let Some(tor) = tor_lock.as_ref() {
            let my_onion = tor
                .onion_address()
                .map(|s| s.to_string())
                .unwrap_or_default();
            for peer in peers.values() {
                if let Ok(stream) = tor.connect(&peer.onion_addr).await {
                    let mut framed = FramedStream::new(stream);
                    let hello = HelloPayload {
                        address: self.identity.address.clone(),
                        alias: self.alias.clone(),
                        verifying_key: self.identity.verifying_key.to_bytes(),
                        listen_addr: my_onion.clone(),
                        timestamp: Utc::now(),
                    };
                    let _ = framed.send_json(&WireMessage::Hello(hello)).await;
                    if let Ok(WireMessage::HelloAck(_)) = framed.recv_json().await {
                        let _ = framed.send_json(&msg).await;
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn fetch_user_timeline(&self, user_address: &str) -> Vec<Message> {
        let key = Dht::timeline_key(user_address);
        if let Some(values) = self.dht.get_value(&key).await {
            values
                .into_iter()
                .filter_map(|v| {
                    if let DhtValue::Post(stored) = v {
                        serde_json::from_slice(&stored.content).ok()
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    pub async fn peer_count(&self) -> usize {
        self.peers.read().await.len()
    }

    pub async fn peer_verifying_key(&self, address: &str) -> Option<[u8; 32]> {
        let peers = self.peers.read().await;
        peers.get(address).map(|p| p.verifying_key)
    }

    pub async fn announce_self(&self) {
        let onion = {
            let tor_lock = self.tor.read().await;
            tor_lock
                .as_ref()
                .and_then(|t| t.onion_address().map(|s| s.to_string()))
                .unwrap_or_default()
        };
        if onion.is_empty() {
            return;
        }

        let announce = StoredPeerAnnounce {
            address: self.identity.address.clone(),
            alias: self.alias.clone(),
            onion_addr: onion,
            timestamp: Utc::now(),
        };

        let key = Dht::peer_registry_key();
        self.dht
            .store_value(&key, DhtValue::PeerAnnounce(announce.clone()))
            .await;

        // Replicate announcement to closest DHT nodes
        let closest = self.dht.find_closest(&key).await;
        let tor_lock = self.tor.read().await;
        if let Some(tor) = tor_lock.as_ref() {
            for node in closest {
                if node.address == self.identity.address {
                    continue;
                }
                if let Ok(stream) = tor.connect(&node.onion_addr).await {
                    let mut framed = FramedStream::new(stream);
                    let store_req = WireMessage::DhtRequest {
                        request_id: rand::random(),
                        request: super::dht::DhtRequest::Store {
                            key: key.clone(),
                            value: DhtValue::PeerAnnounce(announce.clone()),
                        },
                    };
                    let _ = framed.send_json(&store_req).await;
                }
            }
        }

        info!("Announced self to DHT peer registry");
    }

    pub async fn discover_peers(&self) {
        let key = Dht::peer_registry_key();

        // Check local DHT storage first
        let mut discovered: Vec<StoredPeerAnnounce> = Vec::new();
        if let Some(values) = self.dht.get_value(&key).await {
            for v in values {
                if let DhtValue::PeerAnnounce(pa) = v {
                    if pa.address != self.identity.address {
                        discovered.push(pa);
                    }
                }
            }
        }

        // Also query closest DHT nodes for their peer registries
        let closest = self.dht.find_closest(&key).await;
        let tor_lock = self.tor.read().await;
        if let Some(tor) = tor_lock.as_ref() {
            for node in &closest {
                if node.address == self.identity.address {
                    continue;
                }
                if let Ok(stream) = tor.connect(&node.onion_addr).await {
                    let mut framed = FramedStream::new(stream);
                    let find_req = WireMessage::DhtRequest {
                        request_id: rand::random(),
                        request: super::dht::DhtRequest::FindValue { key: key.clone() },
                    };
                    if framed.send_json(&find_req).await.is_ok() {
                        if let Ok(WireMessage::DhtResponse {
                            response: super::dht::DhtResponse::ValueFound(values),
                            ..
                        }) = framed.recv_json::<WireMessage>().await
                        {
                            for v in values {
                                if let DhtValue::PeerAnnounce(pa) = v {
                                    if pa.address != self.identity.address {
                                        discovered.push(pa);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        drop(tor_lock);

        // Connect to newly discovered peers
        for peer in discovered {
            let existing = self.peers.read().await;
            if existing.contains_key(&peer.address) {
                continue;
            }
            drop(existing);

            // Add to routing table
            self.dht
                .add_node(DhtNode {
                    id: NodeId::from_address(&peer.address),
                    address: peer.address.clone(),
                    onion_addr: peer.onion_addr.clone(),
                    last_seen: Utc::now(),
                })
                .await;

            info!("Discovered peer via DHT: {} ({})", peer.alias, peer.address);
            if let Err(e) = self.connect_to(&peer.onion_addr).await {
                warn!("Failed to connect to discovered peer {}: {}", peer.alias, e);
            }
        }
    }

    pub async fn run_discovery_loop(self: Arc<Self>) {
        // Wait for Tor to be ready
        loop {
            {
                let tor_lock = self.tor.read().await;
                if tor_lock.as_ref().and_then(|t| t.onion_address()).is_some() {
                    break;
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }

        // Initial announce + discovery after a short delay
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        self.announce_self().await;
        self.discover_peers().await;
        self.request_peer_lists().await;
        self.sync_timeline_from_peers().await;

        // Re-announce, discover, and check for pending DMs periodically
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(120));
        loop {
            interval.tick().await;
            self.announce_self().await;
            self.discover_peers().await;
            self.request_peer_lists().await;
            self.fetch_pending_dms().await;
        }
    }

    pub async fn connect_to_seeds(&self) {
        // Wait for Tor to be ready
        loop {
            {
                let tor_lock = self.tor.read().await;
                if tor_lock.as_ref().and_then(|t| t.onion_address()).is_some() {
                    break;
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }

        let mut seeds: Vec<String> = SEED_NODES.iter().map(|s| s.to_string()).collect();

        // Also load seeds from env var (comma-separated)
        if let Ok(extra) = std::env::var("Y_SEEDS") {
            for s in extra.split(',') {
                let trimmed = s.trim().to_string();
                if !trimmed.is_empty() && !seeds.contains(&trimmed) {
                    seeds.push(trimmed);
                }
            }
        }

        if seeds.is_empty() {
            info!("No seed nodes configured");
            return;
        }

        // Stagger connections slightly to avoid overwhelming Tor
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;

        for seed in &seeds {
            info!("Connecting to seed node: {}", seed);
            match self.connect_to(seed).await {
                Ok(()) => info!("Connected to seed node: {}", seed),
                Err(e) => warn!("Failed to connect to seed {}: {}", seed, e),
            }
        }
    }

    pub async fn run_health_check_loop(self: Arc<Self>) {
        // Wait for Tor to bootstrap first
        loop {
            {
                let tor_lock = self.tor.read().await;
                if tor_lock.as_ref().and_then(|t| t.onion_address()).is_some() {
                    break;
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }

        let mut was_online = true;
        let mut fail_count: u32 = 0;
        let _ = self.event_tx.send(NetworkEvent::ConnectivityChanged(true));

        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            interval.tick().await;

            let check_passed = self.check_connectivity().await;

            if check_passed {
                fail_count = 0;
                if !was_online {
                    was_online = true;
                    let _ = self.event_tx.send(NetworkEvent::ConnectivityChanged(true));
                    info!("Connectivity restored — syncing");
                    self.announce_self().await;
                    self.discover_peers().await;
                    self.request_peer_lists().await;
                    self.fetch_pending_dms().await;
                }

                // Evict dead peers by pinging each one
                self.evict_dead_peers().await;
            } else {
                fail_count += 1;
                // Only mark offline after 3 consecutive failures (~90s)
                if was_online && fail_count >= 3 {
                    was_online = false;
                    let _ = self.event_tx.send(NetworkEvent::ConnectivityChanged(false));
                    info!("Connectivity lost");
                }
            }
        }
    }

    async fn evict_dead_peers(&self) {
        let peer_info: Vec<(String, String)> = {
            let peers = self.peers.read().await;
            peers
                .values()
                .map(|p| (p.address.clone(), p.onion_addr.clone()))
                .collect()
        };

        let tor_lock = self.tor.read().await;
        let tor = match tor_lock.as_ref() {
            Some(t) => t,
            None => return,
        };

        let mut dead: Vec<String> = Vec::new();
        for (address, onion) in &peer_info {
            let reachable = if let Ok(Ok(stream)) =
                tokio::time::timeout(std::time::Duration::from_secs(15), tor.connect(onion)).await
            {
                let mut framed = FramedStream::new(stream);
                let nonce: u64 = rand::random();
                if framed.send_json(&WireMessage::Ping(nonce)).await.is_ok() {
                    tokio::time::timeout(
                        std::time::Duration::from_secs(10),
                        framed.recv_json::<WireMessage>(),
                    )
                    .await
                    .is_ok()
                } else {
                    false
                }
            } else {
                false
            };

            if !reachable {
                dead.push(address.clone());
            }
        }
        drop(tor_lock);

        if !dead.is_empty() {
            let mut peers = self.peers.write().await;
            for address in &dead {
                if let Some(peer) = peers.remove(address) {
                    info!("Evicted dead peer: {} ({})", peer.alias, address);
                }
            }
            let count = peers.len();
            drop(peers);
            let _ = self.event_tx.send(NetworkEvent::PeerCountChanged(count));
            for address in dead {
                let _ = self
                    .event_tx
                    .send(NetworkEvent::PeerDisconnected { address });
            }
        }
    }

    async fn check_connectivity(&self) -> bool {
        // Try pinging any connected peer first
        let peer_addrs: Vec<String> = {
            let peers = self.peers.read().await;
            peers.values().map(|p| p.onion_addr.clone()).collect()
        };

        let tor_lock = self.tor.read().await;
        let tor = match tor_lock.as_ref() {
            Some(t) => t,
            None => return false,
        };

        for addr in peer_addrs.iter().take(3) {
            if let Ok(Ok(stream)) =
                tokio::time::timeout(std::time::Duration::from_secs(15), tor.connect(addr)).await
            {
                let mut framed = FramedStream::new(stream);
                let nonce: u64 = rand::random();
                if framed.send_json(&WireMessage::Ping(nonce)).await.is_ok() {
                    if let Ok(Ok(WireMessage::Pong(_))) = tokio::time::timeout(
                        std::time::Duration::from_secs(10),
                        framed.recv_json::<WireMessage>(),
                    )
                    .await
                    {
                        return true;
                    }
                }
            }
        }

        // Fall back to trying seed nodes
        for seed in SEED_NODES {
            if let Ok(stream) =
                tokio::time::timeout(std::time::Duration::from_secs(15), tor.connect(seed)).await
            {
                if stream.is_ok() {
                    return true;
                }
            }
        }

        false
    }

    async fn relay_nod_event(&self, msg: &WireMessage, originator: &str) {
        let peers: Vec<String> = {
            let p = self.peers.read().await;
            p.values()
                .filter(|pc| pc.address != originator)
                .map(|pc| pc.onion_addr.clone())
                .collect()
        };

        let tor_lock = self.tor.read().await;
        if let Some(tor) = tor_lock.as_ref() {
            for onion in peers {
                if let Ok(stream) = tor.connect(&onion).await {
                    let mut framed = FramedStream::new(stream);
                    let my_onion = tor
                        .onion_address()
                        .map(|s| s.to_string())
                        .unwrap_or_default();
                    let hello = HelloPayload {
                        address: self.identity.address.clone(),
                        alias: self.alias.clone(),
                        verifying_key: self.identity.verifying_key.to_bytes(),
                        listen_addr: my_onion,
                        timestamp: Utc::now(),
                    };
                    let _ = framed.send_json(&WireMessage::Hello(hello)).await;
                    if let Ok(WireMessage::HelloAck(_)) = framed.recv_json().await {
                        let _ = framed.send_json(msg).await;
                    }
                }
            }
        }
    }

    async fn relay_post(&self, post: &Message) {
        let msg = WireMessage::BroadcastPost(post.clone());
        let peers: Vec<String> = {
            let p = self.peers.read().await;
            p.values()
                .filter(|pc| pc.address != post.author)
                .map(|pc| pc.onion_addr.clone())
                .collect()
        };

        let tor_lock = self.tor.read().await;
        if let Some(tor) = tor_lock.as_ref() {
            for onion in peers {
                if let Ok(stream) = tor.connect(&onion).await {
                    let mut framed = FramedStream::new(stream);
                    let my_onion = tor
                        .onion_address()
                        .map(|s| s.to_string())
                        .unwrap_or_default();
                    let hello = HelloPayload {
                        address: self.identity.address.clone(),
                        alias: self.alias.clone(),
                        verifying_key: self.identity.verifying_key.to_bytes(),
                        listen_addr: my_onion,
                        timestamp: Utc::now(),
                    };
                    let _ = framed.send_json(&WireMessage::Hello(hello)).await;
                    if let Ok(WireMessage::HelloAck(_)) = framed.recv_json().await {
                        let _ = framed.send_json(&msg).await;
                    }
                }
            }
        }
    }

    pub async fn sync_timeline_from_peers(&self) {
        let peers: Vec<String> = {
            let p = self.peers.read().await;
            p.values().map(|pc| pc.onion_addr.clone()).collect()
        };

        let tor_lock = self.tor.read().await;
        if let Some(tor) = tor_lock.as_ref() {
            for onion in peers {
                if let Ok(stream) = tor.connect(&onion).await {
                    let mut framed = FramedStream::new(stream);
                    let my_onion = tor
                        .onion_address()
                        .map(|s| s.to_string())
                        .unwrap_or_default();
                    let hello = HelloPayload {
                        address: self.identity.address.clone(),
                        alias: self.alias.clone(),
                        verifying_key: self.identity.verifying_key.to_bytes(),
                        listen_addr: my_onion,
                        timestamp: Utc::now(),
                    };
                    let _ = framed.send_json(&WireMessage::Hello(hello)).await;
                    if let Ok(WireMessage::HelloAck(_)) = framed.recv_json().await {
                        let req = WireMessage::RequestTimeline {
                            since: None,
                            limit: 50,
                        };
                        let _ = framed.send_json(&req).await;
                        if let Ok(WireMessage::TimelineResponse(posts)) = framed.recv_json().await {
                            let mut known = self.known_posts.write().await;
                            for post in posts {
                                if !known.contains(&post.id) {
                                    known.push(post.id.clone());
                                    let _ = self.event_tx.send(NetworkEvent::NewPost(post.clone()));
                                    self.dht_store_post(&post).await;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    async fn request_peer_lists(&self) {
        let peers: Vec<String> = {
            let p = self.peers.read().await;
            p.values().map(|pc| pc.onion_addr.clone()).collect()
        };

        let mut to_connect: Vec<(String, String)> = Vec::new();

        let tor_lock = self.tor.read().await;
        if let Some(tor) = tor_lock.as_ref() {
            let my_onion = tor
                .onion_address()
                .map(|s| s.to_string())
                .unwrap_or_default();
            for onion in peers {
                if let Ok(stream) = tor.connect(&onion).await {
                    let mut framed = FramedStream::new(stream);
                    let hello = HelloPayload {
                        address: self.identity.address.clone(),
                        alias: self.alias.clone(),
                        verifying_key: self.identity.verifying_key.to_bytes(),
                        listen_addr: my_onion.clone(),
                        timestamp: Utc::now(),
                    };
                    let _ = framed.send_json(&WireMessage::Hello(hello)).await;
                    if let Ok(WireMessage::HelloAck(_)) = framed.recv_json().await {
                        let _ = framed.send_json(&WireMessage::RequestPeers).await;
                        if let Ok(WireMessage::PeersResponse(new_peers)) = framed.recv_json().await
                        {
                            for peer in new_peers {
                                if peer.address == self.identity.address {
                                    continue;
                                }
                                let existing = self.peers.read().await;
                                if existing.contains_key(&peer.address) {
                                    continue;
                                }
                                drop(existing);

                                self.dht
                                    .add_node(DhtNode {
                                        id: NodeId::from_address(&peer.address),
                                        address: peer.address.clone(),
                                        onion_addr: peer.listen_addr.clone(),
                                        last_seen: Utc::now(),
                                    })
                                    .await;

                                to_connect.push((peer.alias.clone(), peer.listen_addr.clone()));

                                info!(
                                    "Discovered peer via mediator: {} ({})",
                                    peer.alias, peer.address
                                );
                            }
                        }
                    }
                }
            }
        }
        drop(tor_lock);

        for (alias, listen_addr) in to_connect {
            if let Err(e) = self.connect_to(&listen_addr).await {
                warn!("Failed to connect to discovered peer {}: {}", alias, e);
            }
        }
    }
}
