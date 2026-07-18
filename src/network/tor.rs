use anyhow::Result;
use arti_client::config::TorClientConfigBuilder;
use arti_client::{DataStream, TorClient};
use futures::StreamExt;
use std::sync::Arc;
use tokio::sync::mpsc;
use tor_cell::relaycell::msg::Connected;
use tor_hsservice::config::OnionServiceConfigBuilder;
use tor_hsservice::{RendRequest, RunningOnionService};
use tor_rtcompat::PreferredRuntime;
use tracing::{info, warn};

pub struct TorTransport {
    client: TorClient<PreferredRuntime>,
    _onion_service: Option<Arc<RunningOnionService>>,
    onion_address: Option<String>,
}

impl TorTransport {
    pub async fn bootstrap(data_dir: &std::path::Path) -> Result<Self> {
        let tor_dir = data_dir.join("tor");
        std::fs::create_dir_all(&tor_dir)?;

        info!("Bootstrapping Tor client (this may take a moment on first run)...");

        let config =
            TorClientConfigBuilder::from_directories(tor_dir.join("state"), tor_dir.join("cache"))
                .build()
                .map_err(|e| anyhow::anyhow!("Tor config build error: {}", e))?;

        let client = TorClient::create_bootstrapped(config)
            .await
            .map_err(|e| anyhow::anyhow!("Tor bootstrap error: {}", e))?;

        info!("Tor client bootstrapped successfully");

        Ok(Self {
            client,
            _onion_service: None,
            onion_address: None,
        })
    }

    pub async fn start_hidden_service(
        &mut self,
        port: u16,
    ) -> Result<mpsc::UnboundedReceiver<DataStream>> {
        let nickname = "y-chat"
            .parse()
            .map_err(|e| anyhow::anyhow!("Bad nickname: {}", e))?;

        let svc_config = OnionServiceConfigBuilder::default()
            .nickname(nickname)
            .build()
            .map_err(|e| anyhow::anyhow!("Onion service config error: {}", e))?;

        let (service, rend_requests) = self
            .client
            .launch_onion_service(svc_config)
            .map_err(|e| anyhow::anyhow!("Failed to launch onion service: {}", e))?;

        let onion_addr = service
            .onion_name()
            .map(|hsid| format!("{}.onion:{}", hsid, port))
            .unwrap_or_else(|| "pending.onion".to_string());

        info!("Hidden service running at: {}", onion_addr);
        self.onion_address = Some(onion_addr);
        self._onion_service = Some(service);

        let (tx, rx) = mpsc::unbounded_channel();

        tokio::spawn(Self::accept_loop(rend_requests, tx));

        Ok(rx)
    }

    async fn accept_loop(
        mut rend_requests: impl futures::Stream<Item = RendRequest> + Unpin + Send + 'static,
        tx: mpsc::UnboundedSender<DataStream>,
    ) {
        while let Some(rend_request) = rend_requests.next().await {
            let tx = tx.clone();
            tokio::spawn(async move {
                match rend_request.accept().await {
                    Ok(mut stream_requests) => {
                        while let Some(stream_request) = stream_requests.next().await {
                            match stream_request.accept(Connected::new_empty()).await {
                                Ok(stream) => {
                                    let _ = tx.send(stream);
                                }
                                Err(e) => warn!("Failed to accept stream: {}", e),
                            }
                        }
                    }
                    Err(e) => warn!("Failed to accept rendezvous: {}", e),
                }
            });
        }
    }

    pub async fn connect(&self, onion_addr: &str) -> Result<DataStream> {
        let stream = self
            .client
            .connect(onion_addr)
            .await
            .map_err(|e| anyhow::anyhow!("Tor connect error: {}", e))?;
        Ok(stream)
    }

    pub fn onion_address(&self) -> Option<&str> {
        self.onion_address.as_deref()
    }
}
