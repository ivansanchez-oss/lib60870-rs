mod task101;
mod task104;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::asdu::Asdu;
use crate::client::ConnectionState;
use crate::error::RequestError;
use crate::ft12::LinkAddress;
use crate::transport::{
    Listener, SerialOverTcpListener, SerialOverTcpListenerConfig, TcpListener, TcpListenerConfig,
};
use crate::types::{ApciParameters, AppLayerParameters, LinkLayerParameters};

// --- ServerHandler trait ---

/// How the server should respond to a received ASDU from the master.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsduResponse {
    /// Positive confirmation (P/N bit = positive).
    Confirm,
    /// Negative confirmation (P/N bit = negative).
    Negative,
}

/// Trait for receiving events from a server / slave session.
///
/// The associated type [`Param`](ServerHandler::Param) allows passing
/// user-defined context to every callback. Use `()` if no extra context
/// is needed.
pub trait ServerHandler: Send + 'static {
    /// User-defined parameter passed to every callback.
    type Param: Send;

    /// Called when the transport connection state changes.
    fn on_connection_state(&mut self, state: ConnectionState, param: &mut Self::Param);

    /// Called for each ASDU received from the master (interrogation, commands, etc.).
    ///
    /// Return [`AsduResponse::Confirm`] to send a positive activation confirmation,
    /// or [`AsduResponse::Negative`] for a negative one.
    ///
    /// For interrogation requests, the handler should enqueue response ASDUs via
    /// the [`ServerHandle`] before returning.
    fn on_asdu(&mut self, asdu: &Asdu, param: &mut Self::Param) -> AsduResponse;
}

// --- ServerHandle ---

/// Priority class for enqueued ASDUs.
///
/// For CS101 (FT 1.2), class 1 data is delivered via `RequestClass1` polls
/// and sets the ACD bit. Class 2 data is delivered via `RequestClass2` polls.
///
/// For CS104, both classes are sent as I-frames (class 1 has priority).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventClass {
    /// High priority (spontaneous events, interrogation responses).
    Class1,
    /// Low priority (background scan, periodic).
    Class2,
}

pub(crate) enum ServerCommand {
    EnqueueAsdu {
        asdu: Asdu,
        class: EventClass,
    },
    Shutdown,
}

/// Handle to interact with a running server / slave task.
///
/// Lightweight and cloneable. Used from the main thread to enqueue
/// pre-built ASDUs for transmission to the master.
#[derive(Clone)]
pub struct ServerHandle {
    tx: mpsc::Sender<ServerCommand>,
    connected: Arc<AtomicBool>,
}

impl ServerHandle {
    fn new(tx: mpsc::Sender<ServerCommand>, connected: Arc<AtomicBool>) -> Self {
        Self { tx, connected }
    }

    /// Returns `true` if a master is currently connected.
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Acquire)
    }

    /// Enqueue a pre-built ASDU for transmission to the master.
    ///
    /// For CS101, class 1 ASDUs are delivered when the master polls class 1
    /// and cause the ACD bit to be set. Class 2 ASDUs are delivered on
    /// class 2 polls.
    ///
    /// For CS104, ASDUs are sent as I-frames when data transfer is active.
    /// Class 1 ASDUs have priority over class 2.
    pub async fn enqueue_asdu(
        &self,
        asdu: Asdu,
        class: EventClass,
    ) -> Result<(), RequestError> {
        self.tx
            .send(ServerCommand::EnqueueAsdu { asdu, class })
            .await
            .map_err(|_| RequestError::TaskClosed)
    }

    /// Gracefully shut down the server task.
    pub async fn shutdown(&self) -> Result<(), RequestError> {
        let _ = self.tx.send(ServerCommand::Shutdown).await;
        Ok(())
    }
}

// --- CS104 Server ---

/// Configuration for a CS104 server.
#[derive(Debug, Clone)]
pub struct Server104Config {
    pub apci: ApciParameters,
    pub app: AppLayerParameters,
    pub max_connections: usize,
}

impl Default for Server104Config {
    fn default() -> Self {
        Self {
            apci: ApciParameters::default(),
            app: AppLayerParameters::CS104_DEFAULT,
            max_connections: 1,
        }
    }
}

/// Listener transport configuration for the CS104 server.
pub enum ListenerConfig104 {
    /// Plain TCP listener.
    Tcp(TcpListenerConfig),
    /// TLS listener.
    #[cfg(feature = "tls")]
    Tls(crate::transport::TlsListenerConfig),
}

/// Entry point for creating a CS104 server.
///
/// Bundles listener, protocol configuration, handler and user parameter.
/// Call [`run()`](Server104::run) to bind and start accepting connections.
pub struct Server104<H: ServerHandler> {
    listener_config: ListenerConfig104,
    config: Server104Config,
    handler: H,
    param: H::Param,
}

impl<H: ServerHandler> Server104<H> {
    pub fn new(
        listener_config: ListenerConfig104,
        config: Server104Config,
        handler: H,
        param: H::Param,
    ) -> Self {
        Self {
            listener_config,
            config,
            handler,
            param,
        }
    }

    /// Bind the listener and start accepting master connections.
    ///
    /// Returns a [`ServerHandle`] for enqueuing ASDUs and controlling the server.
    /// The server task runs in the background until [`ServerHandle::shutdown()`] is called.
    pub fn run(self) -> ServerHandle {
        let (tx, rx) = mpsc::channel(64);
        let connected = Arc::new(AtomicBool::new(false));
        let handle = ServerHandle::new(tx, connected.clone());

        match self.listener_config {
            ListenerConfig104::Tcp(cfg) => {
                let task = ServerTask104 {
                    config: self.config,
                    handler: self.handler,
                    param: self.param,
                    commands: rx,
                    connected,
                };
                tokio::spawn(task.run_tcp(cfg));
            }
            #[cfg(feature = "tls")]
            ListenerConfig104::Tls(cfg) => {
                let task = ServerTask104 {
                    config: self.config,
                    handler: self.handler,
                    param: self.param,
                    commands: rx,
                    connected,
                };
                tokio::spawn(task.run_tls(cfg));
            }
        }

        handle
    }
}

struct ServerTask104<H: ServerHandler> {
    config: Server104Config,
    handler: H,
    param: H::Param,
    commands: mpsc::Receiver<ServerCommand>,
    connected: Arc<AtomicBool>,
}

impl<H: ServerHandler> ServerTask104<H> {
    async fn run_tcp(self, listener_config: TcpListenerConfig) {
        let listener = match TcpListener::bind(listener_config).await {
            Ok(l) => l,
            Err(e) => {
                warn!(?e, "failed to bind TCP listener");
                return;
            }
        };
        self.accept_loop(listener).await;
    }

    #[cfg(feature = "tls")]
    async fn run_tls(self, listener_config: crate::transport::TlsListenerConfig) {
        let listener = match crate::transport::TlsListener::bind(listener_config).await {
            Ok(l) => l,
            Err(e) => {
                warn!(?e, "failed to bind TLS listener");
                return;
            }
        };
        self.accept_loop(listener).await;
    }

    async fn accept_loop<L: Listener>(mut self, listener: L) {
        loop {
            tokio::select! {
                accept_result = listener.accept() => {
                    match accept_result {
                        Ok((phys, addr)) => {
                            if self.connected.load(Ordering::Acquire) {
                                info!(%addr, "rejecting connection: max connections reached");
                                drop(phys);
                                continue;
                            }
                            info!(%addr, "master connected");
                            task104::run_session(
                                &self.config,
                                &mut self.handler,
                                &mut self.param,
                                &mut self.commands,
                                phys,
                                &self.connected,
                            ).await;
                        }
                        Err(e) => {
                            warn!(?e, "accept error");
                        }
                    }
                }

                cmd = self.commands.recv() => {
                    match cmd {
                        Some(ServerCommand::Shutdown) | None => {
                            info!("server shutting down");
                            return;
                        }
                        _ => {
                            // Enqueue commands are ignored when no session is active
                        }
                    }
                }
            }
        }
    }
}

// --- CS101 Slave ---

/// Configuration for a CS101 slave.
#[derive(Debug, Clone)]
pub struct Slave101Config {
    pub link: LinkLayerParameters,
    pub app: AppLayerParameters,
    pub link_address: LinkAddress,
}

impl Default for Slave101Config {
    fn default() -> Self {
        Self {
            link: LinkLayerParameters::default(),
            app: AppLayerParameters::CS101_DEFAULT,
            link_address: LinkAddress(1),
        }
    }
}

/// Listener transport configuration for the CS101 slave.
pub enum ListenerConfig101 {
    /// Serial-over-TCP listener (raw TCP with FT 1.2 framing).
    SerialOverTcp(SerialOverTcpListenerConfig),
    /// Native serial port.
    #[cfg(feature = "serial")]
    Serial(crate::transport::SerialConfig),
}

/// Entry point for creating a CS101 slave.
///
/// Bundles listener, protocol configuration, handler and user parameter.
/// Call [`run()`](Slave101::run) to bind and start accepting connections.
pub struct Slave101<H: ServerHandler> {
    listener_config: ListenerConfig101,
    config: Slave101Config,
    handler: H,
    param: H::Param,
}

impl<H: ServerHandler> Slave101<H> {
    pub fn new(
        listener_config: ListenerConfig101,
        config: Slave101Config,
        handler: H,
        param: H::Param,
    ) -> Self {
        Self {
            listener_config,
            config,
            handler,
            param,
        }
    }

    /// Bind the listener and start accepting master connections.
    ///
    /// Returns a [`ServerHandle`] for enqueuing ASDUs and controlling the slave.
    pub fn run(self) -> ServerHandle {
        let (tx, rx) = mpsc::channel(64);
        let connected = Arc::new(AtomicBool::new(false));
        let handle = ServerHandle::new(tx, connected.clone());

        match self.listener_config {
            ListenerConfig101::SerialOverTcp(cfg) => {
                let task = SlaveTask101 {
                    config: self.config,
                    handler: self.handler,
                    param: self.param,
                    commands: rx,
                    connected,
                };
                tokio::spawn(task.run_serial_over_tcp(cfg));
            }
            #[cfg(feature = "serial")]
            ListenerConfig101::Serial(_cfg) => {
                todo!("native serial port slave not yet implemented");
            }
        }

        handle
    }
}

struct SlaveTask101<H: ServerHandler> {
    config: Slave101Config,
    handler: H,
    param: H::Param,
    commands: mpsc::Receiver<ServerCommand>,
    connected: Arc<AtomicBool>,
}

impl<H: ServerHandler> SlaveTask101<H> {
    async fn run_serial_over_tcp(mut self, listener_config: SerialOverTcpListenerConfig) {
        let listener = match SerialOverTcpListener::bind(listener_config).await {
            Ok(l) => l,
            Err(e) => {
                warn!(?e, "failed to bind serial-over-TCP listener");
                return;
            }
        };

        loop {
            tokio::select! {
                accept_result = listener.accept() => {
                    match accept_result {
                        Ok((phys, addr)) => {
                            if self.connected.load(Ordering::Acquire) {
                                info!(%addr, "rejecting connection: already connected");
                                drop(phys);
                                continue;
                            }
                            info!(%addr, "master connected");
                            let (mut reader, mut writer) = tokio::io::split(phys);
                            task101::run_session(
                                &self.config,
                                &mut self.handler,
                                &mut self.param,
                                &mut self.commands,
                                &mut reader,
                                &mut writer,
                                &self.connected,
                            ).await;
                        }
                        Err(e) => {
                            warn!(?e, "accept error");
                        }
                    }
                }

                cmd = self.commands.recv() => {
                    match cmd {
                        Some(ServerCommand::Shutdown) | None => {
                            info!("slave shutting down");
                            return;
                        }
                        _ => {
                            // Enqueue commands ignored when no session active
                        }
                    }
                }
            }
        }
    }
}
