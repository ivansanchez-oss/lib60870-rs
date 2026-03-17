mod task;

use std::io;

use tokio::sync::{mpsc, oneshot};

use crate::asdu::{Asdu, AsduBuilder, InformationObjectAddress};
use crate::error::Error;
use crate::info::{
    ClockSyncCommand, CounterInterrogationCommand, InformationObject, InterrogationCommand,
    ReadCommand,
};

use crate::transport::{
    RetryStrategy, SerialOverTcpConfig, SerialOverTcpConnector, TcpConfig, TcpConnector,
};

use crate::types::{ApciParameters, AppLayerParameters, CauseOfTransmission, Cp56Time2a};

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
pub trait ClientHandler: Send + 'static {
    /// Called when the transport connection state changes.
    fn on_connection_state(&mut self, state: ConnectionState);

    /// Called for each ASDU received from the server (spontaneous, interrogation responses, etc.).
    fn on_asdu(&mut self, asdu: &Asdu);
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
#[derive(Clone)]
pub struct ClientHandle {
    tx: mpsc::Sender<ClientCommand>,
}

pub(crate) enum ClientCommand {
    SendAsdu {
        asdu: Asdu,
        response: oneshot::Sender<Result<(), io::Error>>,
    },
    Shutdown {
        response: oneshot::Sender<()>,
    },
}

impl ClientHandle {
    /// Send a station interrogation command.
    pub async fn interrogation(&self, ca: u16, qoi: u8) -> Result<(), Error> {
        let asdu = AsduBuilder::new(CauseOfTransmission::Activation, ca)
            .add(
                InformationObjectAddress::new(0),
                InformationObject::Interrogation(InterrogationCommand::new(qoi)),
            )?
            .build()?;
        self.send_asdu(asdu).await
    }

    /// Send a counter interrogation command.
    pub async fn counter_interrogation(&self, ca: u16, qcc: u8) -> Result<(), Error> {
        let asdu = AsduBuilder::new(CauseOfTransmission::Activation, ca)
            .add(
                InformationObjectAddress::new(0),
                InformationObject::CounterInterrogation(CounterInterrogationCommand::new(qcc)),
            )?
            .build()?;
        self.send_asdu(asdu).await
    }

    /// Send a read command for a specific information object address.
    pub async fn read(&self, ca: u16, ioa: u32) -> Result<(), Error> {
        let asdu = AsduBuilder::new(CauseOfTransmission::Request, ca)
            .add(
                InformationObjectAddress::new(ioa),
                InformationObject::Read(ReadCommand),
            )?
            .build()?;
        self.send_asdu(asdu).await
    }

    /// Send a clock synchronization command.
    pub async fn clock_sync(&self, ca: u16, time: Cp56Time2a) -> Result<(), Error> {
        let asdu = AsduBuilder::new(CauseOfTransmission::Activation, ca)
            .add(
                InformationObjectAddress::new(0),
                InformationObject::ClockSync(ClockSyncCommand::new(time)),
            )?
            .build()?;
        self.send_asdu(asdu).await
    }

    /// Send an arbitrary pre-built ASDU (e.g. control commands).
    pub async fn send_command(&self, asdu: Asdu) -> Result<(), Error> {
        self.send_asdu(asdu).await
    }

    /// Gracefully shut down the client session.
    pub async fn shutdown(&self) -> Result<(), Error> {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send(ClientCommand::Shutdown { response: tx }).await;
        let _ = rx.await;
        Ok(())
    }

    async fn send_asdu(&self, asdu: Asdu) -> Result<(), Error> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ClientCommand::SendAsdu { asdu, response: tx })
            .await
            .map_err(|_| Error::Connection("client task closed".into()))?;
        rx.await
            .map_err(|_| Error::Connection("client task closed".into()))?
            .map_err(Error::Io)
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
/// Bundles transport, protocol configuration, and handler into a single
/// object. Call [`run()`](Client104::run) to spawn the async task and
/// obtain a [`ClientHandle`].
pub struct Client104<H> {
    transport: TransportConfig,
    config: ClientConfig,
    handler: H,
}

impl<H: ClientHandler> Client104<H> {
    pub fn new(transport: TransportConfig, config: ClientConfig, handler: H) -> Self {
        Self {
            transport,
            config,
            handler,
        }
    }

    /// Spawn the client task and return a handle for sending commands.
    ///
    /// The task connects using the configured transport, manages APCI, and
    /// delivers ASDUs to the handler. It reconnects automatically using the
    /// configured [`RetryStrategy`].
    pub fn run(self) -> ClientHandle {
        let (tx, rx) = mpsc::channel(64);
        match self.transport {
            TransportConfig::Tcp(cfg) => {
                let connector = TcpConnector::new(cfg);
                let t = task::ClientTask::new(connector, self.config, self.handler, rx);
                tokio::spawn(t.run());
            }
            #[cfg(feature = "tls")]
            TransportConfig::Tls(cfg) => {
                let connector = TlsConnector::new(cfg);
                let t = task::ClientTask::new(connector, self.config, self.handler, rx);
                tokio::spawn(t.run());
            }
            TransportConfig::SerialOverTcp(cfg) => {
                let connector = SerialOverTcpConnector::new(cfg);
                let t = task::ClientTask::new(connector, self.config, self.handler, rx);
                tokio::spawn(t.run());
            }
            #[cfg(feature = "serial")]
            TransportConfig::Serial(cfg) => {
                let connector = SerialConnector::new(cfg);
                let t = task::ClientTask::new(connector, self.config, self.handler, rx);
                tokio::spawn(t.run());
            }
        }
        ClientHandle { tx }
    }
}
