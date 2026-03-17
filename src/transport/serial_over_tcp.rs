use std::io;
use std::net::SocketAddr;

use tokio::net::TcpStream;
use tracing::info;

use super::config::SerialOverTcpConfig;
use super::{Connector, Listener, PhysLayer};

/// Connector for serial-over-TCP (raw TCP without APCI framing).
///
/// Semantically distinct from [`super::TcpConnector`]: the protocol layer
/// running on top uses link layer FT 1.2 framing, not APCI.
#[derive(Debug, Clone)]
pub struct SerialOverTcpConnector {
    pub config: SerialOverTcpConfig,
}

impl SerialOverTcpConnector {
    pub fn new(config: SerialOverTcpConfig) -> Self {
        Self { config }
    }
}

impl Connector for SerialOverTcpConnector {
    async fn connect(&self) -> io::Result<PhysLayer> {
        let stream = tokio::time::timeout(
            self.config.connect_timeout,
            TcpStream::connect(self.config.remote_addr),
        )
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "TCP connect timeout"))??;

        stream.set_nodelay(true)?;
        info!(addr = %self.config.remote_addr, "serial-over-TCP connection established");
        Ok(PhysLayer::new(stream))
    }
}

/// Serial-over-TCP listener (server side, raw TCP with FT 1.2 framing).
pub struct SerialOverTcpListener {
    inner: tokio::net::TcpListener,
}

impl Listener for SerialOverTcpListener {
    type Config = super::config::SerialOverTcpListenerConfig;

    async fn bind(config: Self::Config) -> io::Result<Self> {
        let inner = tokio::net::TcpListener::bind(config.local_addr).await?;
        info!(addr = %config.local_addr, "serial-over-TCP listener bound");
        Ok(Self { inner })
    }

    async fn accept(&self) -> io::Result<(PhysLayer, SocketAddr)> {
        let (stream, addr) = self.inner.accept().await?;
        stream.set_nodelay(true)?;
        info!(%addr, "serial-over-TCP connection accepted");
        Ok((PhysLayer::new(stream), addr))
    }
}
