use std::io;
use std::net::SocketAddr;

use tokio::net::TcpStream;
use tracing::info;

use super::config::TcpConfig;
use super::{Connector, Listener, PhysLayer};

/// Connector that establishes plain TCP connections.
#[derive(Debug, Clone)]
pub struct TcpConnector {
    pub config: TcpConfig,
}

impl TcpConnector {
    pub fn new(config: TcpConfig) -> Self {
        Self { config }
    }
}

impl Connector for TcpConnector {
    async fn connect(&self) -> io::Result<PhysLayer> {
        let stream = tokio::time::timeout(
            self.config.connect_timeout,
            TcpStream::connect(self.config.remote_addr),
        )
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "TCP connect timeout"))??;

        stream.set_nodelay(true)?;
        info!(addr = %self.config.remote_addr, "TCP connection established");
        Ok(PhysLayer::new(stream))
    }
}

/// TCP listener that accepts incoming connections.
pub struct TcpListener {
    inner: tokio::net::TcpListener,
}

impl Listener for TcpListener {
    type Config = super::config::TcpListenerConfig;

    async fn bind(config: Self::Config) -> io::Result<Self> {
        let inner = tokio::net::TcpListener::bind(config.local_addr).await?;
        info!(addr = %config.local_addr, "TCP listener bound");
        Ok(Self { inner })
    }

    async fn accept(&self) -> io::Result<(PhysLayer, SocketAddr)> {
        let (stream, addr) = self.inner.accept().await?;
        stream.set_nodelay(true)?;
        info!(%addr, "TCP connection accepted");
        Ok((PhysLayer::new(stream), addr))
    }
}

/// Connector that establishes TLS-over-TCP connections.
#[cfg(feature = "tls")]
pub struct TlsConnector {
    pub config: super::config::TlsConfig,
}

#[cfg(feature = "tls")]
impl TlsConnector {
    pub fn new(config: super::config::TlsConfig) -> Self {
        Self { config }
    }
}

#[cfg(feature = "tls")]
impl Connector for TlsConnector {
    async fn connect(&self) -> io::Result<PhysLayer> {
        let stream = tokio::time::timeout(
            self.config.tcp.connect_timeout,
            TcpStream::connect(self.config.tcp.remote_addr),
        )
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "TCP connect timeout"))??;

        stream.set_nodelay(true)?;

        let tls_stream = self
            .config
            .tls_connector
            .connect(self.config.server_name.clone(), stream)
            .await?;

        info!(
            addr = %self.config.tcp.remote_addr,
            server_name = ?self.config.server_name,
            "TLS connection established"
        );
        Ok(PhysLayer::new(tls_stream))
    }
}

/// TLS listener that accepts incoming TLS connections.
#[cfg(feature = "tls")]
pub struct TlsListener {
    inner: tokio::net::TcpListener,
    tls_acceptor: tokio_rustls::TlsAcceptor,
}

#[cfg(feature = "tls")]
impl Listener for TlsListener {
    type Config = super::config::TlsListenerConfig;

    async fn bind(config: Self::Config) -> io::Result<Self> {
        let inner = tokio::net::TcpListener::bind(config.local_addr).await?;
        info!(addr = %config.local_addr, "TLS listener bound");
        Ok(Self {
            inner,
            tls_acceptor: config.tls_acceptor,
        })
    }

    async fn accept(&self) -> io::Result<(PhysLayer, SocketAddr)> {
        let (stream, addr) = self.inner.accept().await?;
        stream.set_nodelay(true)?;

        let tls_stream = self.tls_acceptor.accept(stream).await?;
        info!(%addr, "TLS connection accepted");
        Ok((PhysLayer::new(tls_stream), addr))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use std::time::Duration;

    #[tokio::test]
    async fn tcp_connector_fails_with_connection_refused() {
        let addr: SocketAddr = "127.0.0.1:1".parse().unwrap();
        let connector = TcpConnector::new(TcpConfig {
            remote_addr: addr,
            connect_timeout: Duration::from_secs(1),
        });

        let result = connector.connect().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn tcp_listener_accept_returns_remote_addr() {
        use super::super::config::TcpListenerConfig;

        // Bind on a random port
        let listener = TcpListener::bind(TcpListenerConfig::new(
            "127.0.0.1:0".parse().unwrap(),
        ))
        .await
        .unwrap();

        let local_addr = listener.inner.local_addr().unwrap();

        // Connect from a client
        let connector = TcpConnector::new(TcpConfig {
            remote_addr: local_addr,
            connect_timeout: Duration::from_secs(1),
        });

        let (accept_result, connect_result) =
            tokio::join!(listener.accept(), connector.connect());

        let (_phys, remote_addr) = accept_result.unwrap();
        let _client_phys = connect_result.unwrap();

        assert_eq!(remote_addr.ip(), "127.0.0.1".parse::<std::net::IpAddr>().unwrap());
        assert_ne!(remote_addr.port(), 0);
    }
}
