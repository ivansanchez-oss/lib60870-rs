mod task;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};

use crate::asdu::{Asdu, AsduBuilder, InformationObjectAddress};
use crate::error::RequestError;
use crate::info::{
    ClockSyncCommand, CounterInterrogationCommand, InformationObject, InterrogationCommand,
    ReadCommand,
};

use crate::transport::{
    RetryStrategy, SerialOverTcpConfig, SerialOverTcpConnector, TcpConfig, TcpConnector,
};

use crate::types::{
    ApciParameters, AppLayerParameters, CauseOfTransmission, CommonAddress, Cp56Time2a,
};

#[cfg(feature = "tls")]
use crate::transport::{TlsConfig, TlsConnector};

#[cfg(feature = "serial")]
use crate::transport::{SerialConfig, SerialConnector};

/// Connection state reported to the [`ClientHandler`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Connected,
    Disconnected,
}

/// Trait for receiving events from the 104 client session.
///
/// The associated type [`Param`](ClientHandler::Param) allows passing
/// user-defined context to every callback. Use `()` if no extra context
/// is needed.
pub trait ClientHandler: Send + 'static {
    /// User-defined parameter passed to every callback.
    type Param: Send;

    /// Called when the transport connection state changes.
    fn on_connection_state(&mut self, state: ConnectionState, param: &mut Self::Param);

    /// Called for each ASDU received from the server (spontaneous, interrogation responses, etc.).
    fn on_asdu(&mut self, asdu: &Asdu, param: &mut Self::Param);
}

/// Protocol-level configuration for a 104 client.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub apci: ApciParameters,
    pub app: AppLayerParameters,
    pub retry: RetryStrategy,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            apci: ApciParameters::default(),
            app: AppLayerParameters::CS104_DEFAULT,
            retry: RetryStrategy::default(),
        }
    }
}

/// Handle to interact with a running 104 client task.
///
/// Lightweight and cloneable. All methods are async and wait until the
/// command is enqueued into the send window (backpressure if window is full).
///
/// Sending commands while disconnected returns [`RequestError::NotConnected`]
/// immediately, matching the behavior of the C lib60870.
#[derive(Clone)]
pub struct ClientHandle {
    tx: mpsc::Sender<ClientCommand>,
    connected: Arc<AtomicBool>,
}

/// A one-shot callback that guarantees the caller always receives a response.
///
/// If the `Promise` is dropped without calling [`complete`](Promise::complete),
/// the `Drop` impl automatically sends `Err(RequestError::Shutdown)`, preventing
/// the receiver from hanging indefinitely (e.g. on task panic or early return).
pub(crate) struct Promise<T> {
    inner: Option<oneshot::Sender<Result<T, RequestError>>>,
}

impl<T> Promise<T> {
    pub fn new() -> (Self, oneshot::Receiver<Result<T, RequestError>>) {
        let (tx, rx) = oneshot::channel();
        (Self { inner: Some(tx) }, rx)
    }

    pub fn complete(mut self, value: Result<T, RequestError>) {
        if let Some(tx) = self.inner.take() {
            let _ = tx.send(value);
        }
    }
}

impl<T> Drop for Promise<T> {
    fn drop(&mut self) {
        if let Some(tx) = self.inner.take() {
            let _ = tx.send(Err(RequestError::Shutdown));
        }
    }
}

pub(crate) enum ClientCommand {
    StartDt {
        promise: Promise<()>,
    },
    StopDt {
        promise: Promise<()>,
    },
    SendAsdu {
        asdu: Asdu,
        promise: Promise<()>,
    },
    Shutdown {
        response: oneshot::Sender<()>,
    },
}

impl ClientHandle {
    /// Returns `true` if the client session is currently connected.
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Acquire)
    }

    /// Send STARTDT activation to begin data transfer.
    ///
    /// Must be called after the transport connects before sending any ASDUs.
    pub async fn start_dt(&self) -> Result<(), RequestError> {
        let (promise, rx) = Promise::new();
        self.tx
            .send(ClientCommand::StartDt { promise })
            .await
            .map_err(|_| RequestError::TaskClosed)?;
        rx.await.map_err(|_| RequestError::Shutdown)?
    }

    /// Send STOPDT activation to pause data transfer.
    ///
    /// After this, the connection remains open but no I-frames are exchanged
    /// until [`start_dt()`](ClientHandle::start_dt) is called again.
    pub async fn stop_dt(&self) -> Result<(), RequestError> {
        let (promise, rx) = Promise::new();
        self.tx
            .send(ClientCommand::StopDt { promise })
            .await
            .map_err(|_| RequestError::TaskClosed)?;
        rx.await.map_err(|_| RequestError::Shutdown)?
    }

    /// Send a station interrogation command.
    pub async fn interrogation(&self, ca: CommonAddress, qoi: u8) -> Result<(), RequestError> {
        let asdu = AsduBuilder::new(CauseOfTransmission::Activation, ca)
            .add(
                InformationObjectAddress::from(0u16),
                InformationObject::Interrogation(InterrogationCommand::new(qoi)),
            )?
            .build()?;
        self.send_asdu(asdu).await
    }

    /// Send a counter interrogation command.
    pub async fn counter_interrogation(&self, ca: CommonAddress, qcc: u8) -> Result<(), RequestError> {
        let asdu = AsduBuilder::new(CauseOfTransmission::Activation, ca)
            .add(
                InformationObjectAddress::from(0u16),
                InformationObject::CounterInterrogation(CounterInterrogationCommand::new(qcc)),
            )?
            .build()?;
        self.send_asdu(asdu).await
    }

    /// Send a read command for a specific information object address.
    pub async fn read(
        &self,
        ca: CommonAddress,
        ioa: InformationObjectAddress,
    ) -> Result<(), RequestError> {
        let asdu = AsduBuilder::new(CauseOfTransmission::Request, ca)
            .add(ioa, InformationObject::Read(ReadCommand))?
            .build()?;
        self.send_asdu(asdu).await
    }

    /// Send a clock synchronization command.
    pub async fn clock_sync(&self, ca: CommonAddress, time: Cp56Time2a) -> Result<(), RequestError> {
        let asdu = AsduBuilder::new(CauseOfTransmission::Activation, ca)
            .add(
                InformationObjectAddress::from(0u16),
                InformationObject::ClockSync(ClockSyncCommand::new(time)),
            )?
            .build()?;
        self.send_asdu(asdu).await
    }

    /// Send an arbitrary pre-built ASDU (e.g. control commands).
    pub async fn send_command(&self, asdu: Asdu) -> Result<(), RequestError> {
        self.send_asdu(asdu).await
    }

    /// Gracefully shut down the client session.
    pub async fn shutdown(&self) -> Result<(), RequestError> {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send(ClientCommand::Shutdown { response: tx }).await;
        let _ = rx.await;
        Ok(())
    }

    async fn send_asdu(&self, asdu: Asdu) -> Result<(), RequestError> {
        if !self.connected.load(Ordering::Acquire) {
            return Err(RequestError::NotConnected);
        }
        let (promise, rx) = Promise::new();
        self.tx
            .send(ClientCommand::SendAsdu { asdu, promise })
            .await
            .map_err(|_| RequestError::TaskClosed)?;
        rx.await.map_err(|_| RequestError::Shutdown)?
    }
}

/// Transport configuration for the 104 client.
///
/// Each variant carries the full configuration needed to create the
/// corresponding [`Connector`] — no separate address field required.
pub enum TransportConfig {
    /// Plain TCP transport.
    Tcp(TcpConfig),
    /// TLS-over-TCP transport.
    #[cfg(feature = "tls")]
    Tls(TlsConfig),
    /// Serial-over-TCP transport (raw TCP with FT 1.2 framing).
    SerialOverTcp(SerialOverTcpConfig),
    /// Native serial port transport.
    #[cfg(feature = "serial")]
    Serial(SerialConfig),
}

/// Entry point for creating a 104 client session.
///
/// Bundles transport, protocol configuration, handler and user parameter
/// into a single object. Call [`run()`](Client104::run) to spawn the
/// async task and obtain a [`ClientHandle`].
pub struct Client104<H: ClientHandler> {
    transport: TransportConfig,
    config: ClientConfig,
    handler: H,
    param: H::Param,
}

impl<H: ClientHandler> Client104<H> {
    pub fn new(
        transport: TransportConfig,
        config: ClientConfig,
        handler: H,
        param: H::Param,
    ) -> Self {
        Self {
            transport,
            config,
            handler,
            param,
        }
    }

    /// Spawn the client task and return a handle for sending commands.
    ///
    /// The task connects using the configured transport, manages APCI, and
    /// delivers ASDUs to the handler. It reconnects automatically using the
    /// configured [`RetryStrategy`].
    pub fn run(self) -> ClientHandle {
        let (tx, rx) = mpsc::channel(64);
        let connected = Arc::new(AtomicBool::new(false));
        let connected_flag = connected.clone();
        match self.transport {
            TransportConfig::Tcp(cfg) => {
                let connector = TcpConnector::new(cfg);
                let t = task::ClientTask::new(
                    connector,
                    self.config,
                    self.handler,
                    self.param,
                    rx,
                    connected_flag,
                );
                tokio::spawn(t.run());
            }
            #[cfg(feature = "tls")]
            TransportConfig::Tls(cfg) => {
                let connector = TlsConnector::new(cfg);
                let t = task::ClientTask::new(
                    connector,
                    self.config,
                    self.handler,
                    self.param,
                    rx,
                    connected_flag,
                );
                tokio::spawn(t.run());
            }
            TransportConfig::SerialOverTcp(cfg) => {
                let connector = SerialOverTcpConnector::new(cfg);
                let t = task::ClientTask::new(
                    connector,
                    self.config,
                    self.handler,
                    self.param,
                    rx,
                    connected_flag,
                );
                tokio::spawn(t.run());
            }
            #[cfg(feature = "serial")]
            TransportConfig::Serial(cfg) => {
                let connector = SerialConnector::new(cfg);
                let t = task::ClientTask::new(
                    connector,
                    self.config,
                    self.handler,
                    self.param,
                    rx,
                    connected_flag,
                );
                tokio::spawn(t.run());
            }
        }
        ClientHandle { tx, connected }
    }
}
