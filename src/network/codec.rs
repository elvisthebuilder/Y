use anyhow::{Result, anyhow};
use tokio::io::{AsyncRead, AsyncWrite, AsyncReadExt, AsyncWriteExt};

const MAX_FRAME_SIZE: u32 = 1024 * 1024; // 1MB max message

pub struct FramedStream<S> {
    stream: S,
}

impl<S: AsyncRead + AsyncWrite + Unpin> FramedStream<S> {
    pub fn new(stream: S) -> Self {
        Self { stream }
    }

    pub async fn send(&mut self, data: &[u8]) -> Result<()> {
        let len = data.len() as u32;
        if len > MAX_FRAME_SIZE {
            return Err(anyhow!("frame too large: {} bytes", len));
        }
        self.stream.write_all(&len.to_be_bytes()).await?;
        self.stream.write_all(data).await?;
        self.stream.flush().await?;
        Ok(())
    }

    pub async fn recv(&mut self) -> Result<Vec<u8>> {
        let mut len_buf = [0u8; 4];
        self.stream.read_exact(&mut len_buf).await?;
        let len = u32::from_be_bytes(len_buf);
        if len > MAX_FRAME_SIZE {
            return Err(anyhow!("frame too large: {} bytes", len));
        }
        let mut buf = vec![0u8; len as usize];
        self.stream.read_exact(&mut buf).await?;
        Ok(buf)
    }

    pub async fn send_json<T: serde::Serialize>(&mut self, msg: &T) -> Result<()> {
        let data = serde_json::to_vec(msg)?;
        self.send(&data).await
    }

    pub async fn recv_json<T: serde::de::DeserializeOwned>(&mut self) -> Result<T> {
        let data = self.recv().await?;
        Ok(serde_json::from_slice(&data)?)
    }
}
