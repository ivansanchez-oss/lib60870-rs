use std::net::SocketAddr;
use std::time::Duration;

/// Configuration for a TCP transport connection.
#[derive(Debug, Clone)]
pub struct TcpConfig {
    pub remote_addr: SocketAddr,
    pub connect_timeout: Duration,
}

impl TcpConfig {
    pub fn new(remote_addr: SocketAddr) -> Self {
        Self {
            remote_addr,
            connect_timeout: Duration::from_secs(10),
        }
    }
}

/// Configuration for TLS transport (TCP + TLS handshake).
#[cfg(feature = "tls")]
pub struct TlsConfig {
    pub tcp: TcpConfig,
    pub tls_connector: tokio_rustls::TlsConnector,
    pub server_name: tokio_rustls::rustls::pki_types::ServerName<'static>,
}

#[cfg(feature = "tls")]
impl std::fmt::Debug for TlsConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TlsConfig")
            .field("tcp", &self.tcp)
            .field("server_name", &self.server_name)
            .finish_non_exhaustive()
    }
}

/// Serial port data bits configuration.
#[cfg(feature = "serial")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataBits {
    Five,
    Six,
    Seven,
    Eight,
}

/// Serial port parity configuration.
#[cfg(feature = "serial")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Parity {
    None,
    Odd,
    Even,
}

/// Serial port stop bits configuration.
#[cfg(feature = "serial")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopBits {
    One,
    Two,
}

/// Configuration for a serial port transport.
#[cfg(feature = "serial")]
#[derive(Debug, Clone)]
pub struct SerialConfig {
    pub path: String,
    pub baud_rate: u32,
    pub data_bits: DataBits,
    pub parity: Parity,
    pub stop_bits: StopBits,
}

/// Configuration for a TCP listener (server side).
#[derive(Debug, Clone)]
pub struct TcpListenerConfig {
    pub local_addr: SocketAddr,
}

impl TcpListenerConfig {
    pub fn new(local_addr: SocketAddr) -> Self {
        Self { local_addr }
    }
}

/// Configuration for a TLS listener (server side).
#[cfg(feature = "tls")]
pub struct TlsListenerConfig {
    pub local_addr: SocketAddr,
    pub tls_acceptor: tokio_rustls::TlsAcceptor,
}

#[cfg(feature = "tls")]
impl std::fmt::Debug for TlsListenerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TlsListenerConfig")
            .field("local_addr", &self.local_addr)
            .finish_non_exhaustive()
    }
}

/// Configuration for a serial-over-TCP listener (server side).
#[derive(Debug, Clone)]
pub struct SerialOverTcpListenerConfig {
    pub local_addr: SocketAddr,
}

impl SerialOverTcpListenerConfig {
    pub fn new(local_addr: SocketAddr) -> Self {
        Self { local_addr }
    }
}

/// Configuration for serial-over-TCP transport.
///
/// Semantically distinct from plain TCP: no APCI framing,
/// only link layer FT 1.2 runs on top.
#[derive(Debug, Clone)]
pub struct SerialOverTcpConfig {
    pub remote_addr: SocketAddr,
    pub connect_timeout: Duration,
}

impl SerialOverTcpConfig {
    pub fn new(remote_addr: SocketAddr) -> Self {
        Self {
            remote_addr,
            connect_timeout: Duration::from_secs(10),
        }
    }
}
