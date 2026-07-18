use anyhow::Result;
use tokio::net::{TcpListener, TcpStream};
use tracing::info;

pub struct TorNode {
    pub onion_address: Option<String>,
    listener: Option<TcpListener>,
}

impl TorNode {
    pub async fn new() -> Result<Self> {
        // In production, this bootstraps a Tor connection via arti-client
        // and creates a hidden service. For now, we set up a local TCP listener
        // that will later be replaced with actual Tor hidden service binding.
        info!("Initializing Tor node...");

        Ok(Self {
            onion_address: None,
            listener: None,
        })
    }

    pub async fn start_hidden_service(&mut self, port: u16) -> Result<()> {
        // Phase 1: Use local TCP for development
        // Phase 2: Replace with arti-client hidden service
        let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).await?;
        let addr = listener.local_addr()?;
        info!("Listening on {} (will be Tor hidden service)", addr);

        self.listener = Some(listener);
        self.onion_address = Some(format!("local:{}", port));

        Ok(())
    }

    pub async fn accept_connection(&self) -> Result<TcpStream> {
        let listener = self.listener.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Hidden service not started"))?;
        let (stream, addr) = listener.accept().await?;
        info!("Peer connected from {}", addr);
        Ok(stream)
    }

    pub async fn connect_to_peer(address: &str) -> Result<TcpStream> {
        // Phase 1: Direct TCP
        // Phase 2: Route through Tor SOCKS proxy to .onion address
        let stream = TcpStream::connect(address).await?;
        Ok(stream)
    }
}
