use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use anyhow::Result;
use arti_client::DataStream;
use chrono::Utc;
use tokio::sync::{mpsc, RwLock};
use tracing::{info, warn};

use crate::crypto::identity::Identity;
use crate::protocol::message::Message;
use super::codec::FramedStream;
use super::protocol::{WireMessage, HelloPayload, PeerAnnounce, EncryptedEnvelope};
use super::tor::TorTransport;

#[derive(Debug, Clone)]
pub enum NetworkEvent {
    PeerConnected { address: String, alias: String },
    PeerDisconnected { address: String },
    NewPost(Message),
    NewDirectMessage(EncryptedEnvelope),
    NodReceived { post_id: String, from: String },
    PeerCountChanged(usize),
    OnionReady(String),
}

pub struct NetworkEngine {
    identity: Identity,
    alias: String,
    listen_port: u16,
    data_dir: PathBuf,
    peers: Arc<RwLock<HashMap<String, PeerConnection>>>,
    event_tx: mpsc::UnboundedSender<NetworkEvent>,
    known_posts: Arc<RwLock<Vec<String>>>,
    tor: Arc<RwLock<Option<TorTransport>>>,
}

struct PeerConnection {
    address: String,
    alias: String,
    onion_addr: String,
}

impl NetworkEngine {
    pub fn new(
        identity: Identity,
        alias: String,
        listen_port: u16,
        data_dir: PathBuf,
        event_tx: mpsc::UnboundedSender<NetworkEvent>,
    ) -> Self {
        Self {
            identity,
            alias,
            listen_port,
            data_dir,
            peers: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
            known_posts: Arc::new(RwLock::new(Vec::new())),
            tor: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn start(self: Arc<Self>) -> Result<()> {
        let mut transport = TorTransport::bootstrap(&self.data_dir).await?;
        let mut incoming = transport.start_hidden_service(self.listen_port).await?;

        if let Some(addr) = transport.onion_address() {
            let _ = self.event_tx.send(NetworkEvent::OnionReady(addr.to_string()));
            info!("Y node reachable at: {}", addr);
        }

        {
            let mut tor_lock = self.tor.write().await;
            *tor_lock = Some(transport);
        }

        info!("Network engine listening via Tor hidden service");

        while let Some(stream) = incoming.recv().await {
            info!("Incoming Tor connection");
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
        let tor = tor_lock.as_ref()
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
            tor_lock.as_ref()
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
        peers.insert(hello.address.clone(), PeerConnection {
            address: hello.address.clone(),
            alias: hello.alias.clone(),
            onion_addr: hello.listen_addr.clone(),
        });
        let count = peers.len();
        drop(peers);

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
                        let _ = self.event_tx.send(NetworkEvent::NewPost(post));
                    }
                }
                WireMessage::RequestTimeline { .. } => {
                    framed.send_json(&WireMessage::TimelineResponse(Vec::new())).await?;
                }
                WireMessage::DirectMessage(envelope) => {
                    if envelope.recipient == self.identity.address {
                        let _ = self.event_tx.send(NetworkEvent::NewDirectMessage(envelope));
                    }
                }
                WireMessage::NodNotify { post_id, from } => {
                    let _ = self.event_tx.send(NetworkEvent::NodReceived { post_id, from });
                }
                WireMessage::RequestPeers => {
                    let peers = self.peers.read().await;
                    let announces: Vec<PeerAnnounce> = peers.values().map(|p| PeerAnnounce {
                        address: p.address.clone(),
                        alias: p.alias.clone(),
                        listen_addr: p.onion_addr.clone(),
                        last_seen: Utc::now(),
                    }).collect();
                    framed.send_json(&WireMessage::PeersResponse(announces)).await?;
                }
                WireMessage::PeersResponse(new_peers) => {
                    for peer in new_peers {
                        let existing = self.peers.read().await;
                        if !existing.contains_key(&peer.address) && peer.address != self.identity.address {
                            drop(existing);
                            info!("Discovered new peer: {} at {}", peer.alias, peer.listen_addr);
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
                                    if let Ok(WireMessage::HelloAck(ack)) = new_framed.recv_json().await {
                                        self.register_peer(&ack).await;
                                    }
                                }
                            }
                        }
                    }
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

    pub async fn broadcast_post(&self, post: &Message) -> Result<()> {
        let mut known = self.known_posts.write().await;
        known.push(post.id.clone());
        drop(known);

        let msg = WireMessage::BroadcastPost(post.clone());
        let data = serde_json::to_vec(&msg)?;

        let peers = self.peers.read().await;
        let tor_lock = self.tor.read().await;
        if let Some(tor) = tor_lock.as_ref() {
            for peer in peers.values() {
                if let Ok(stream) = tor.connect(&peer.onion_addr).await {
                    let mut framed = FramedStream::new(stream);
                    let _ = framed.send(&data).await;
                }
            }
        }
        Ok(())
    }

    pub async fn send_dm(&self, envelope: EncryptedEnvelope) -> Result<()> {
        let msg = WireMessage::DirectMessage(envelope.clone());
        let data = serde_json::to_vec(&msg)?;

        let peers = self.peers.read().await;
        let tor_lock = self.tor.read().await;
        if let Some(tor) = tor_lock.as_ref() {
            if let Some(peer) = peers.get(&envelope.recipient) {
                if let Ok(stream) = tor.connect(&peer.onion_addr).await {
                    let mut framed = FramedStream::new(stream);
                    let _ = framed.send(&data).await;
                    return Ok(());
                }
            }

            for peer in peers.values() {
                if let Ok(stream) = tor.connect(&peer.onion_addr).await {
                    let mut framed = FramedStream::new(stream);
                    let _ = framed.send(&data).await;
                }
            }
        }
        Ok(())
    }

    pub async fn broadcast_nod(&self, post_id: &str) -> Result<()> {
        let msg = WireMessage::NodNotify {
            post_id: post_id.to_string(),
            from: self.identity.address.clone(),
        };
        let data = serde_json::to_vec(&msg)?;

        let peers = self.peers.read().await;
        let tor_lock = self.tor.read().await;
        if let Some(tor) = tor_lock.as_ref() {
            for peer in peers.values() {
                if let Ok(stream) = tor.connect(&peer.onion_addr).await {
                    let mut framed = FramedStream::new(stream);
                    let _ = framed.send(&data).await;
                }
            }
        }
        Ok(())
    }

    pub async fn peer_count(&self) -> usize {
        self.peers.read().await.len()
    }
}
