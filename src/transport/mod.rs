mod config;
mod retry;
mod serial_over_tcp;
mod tcp;

#[cfg(feature = "serial")]
mod serial;

pub use config::*;
pub use retry::{connect_with_retry, RetryStrategy};
pub use serial_over_tcp::{SerialOverTcpConnector, SerialOverTcpListener};
pub use tcp::{TcpConnector, TcpListener};

#[cfg(feature = "tls")]
pub use tcp::{TlsConnector, TlsListener};

#[cfg(feature = "serial")]
pub use serial::SerialConnector;

use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

/// Helper trait combining `AsyncRead + AsyncWrite` for use in trait objects.
trait AsyncReadWrite: AsyncRead + AsyncWrite {}
impl<T: AsyncRead + AsyncWrite> AsyncReadWrite for T {}

/// Type-erased async I/O handle wrapping any `AsyncRead + AsyncWrite` stream.
///
/// Allocated once per connection — cost is negligible.
pub struct PhysLayer {
    inner: Box<dyn AsyncReadWrite + Send + Unpin>,
}

impl PhysLayer {
    pub fn new(io: impl AsyncRead + AsyncWrite + Send + Unpin + 'static) -> Self {
        Self {
            inner: Box::new(io),
        }
    }
}

impl AsyncRead for PhysLayer {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut *self.inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for PhysLayer {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut *self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut *self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut *self.inner).poll_shutdown(cx)
    }
}

/// Factory trait that produces a [`PhysLayer`] on demand (client side).
///
/// Each transport type (TCP, TLS, Serial, etc.) implements this trait.
/// The protocol layer is generic over `C: Connector`.
pub trait Connector: Send + Sync {
    fn connect(&self) -> impl Future<Output = io::Result<PhysLayer>> + Send;
}

/// Server-side trait that binds to a local address and accepts incoming connections.
///
/// Each accepted connection produces a [`PhysLayer`] together with the remote
/// peer's [`SocketAddr`].
pub trait Listener: Send + Sync {
    type Config;

    fn bind(config: Self::Config) -> impl Future<Output = io::Result<Self>> + Send
    where
        Self: Sized;

    fn accept(&self) -> impl Future<Output = io::Result<(PhysLayer, SocketAddr)>> + Send;
}
