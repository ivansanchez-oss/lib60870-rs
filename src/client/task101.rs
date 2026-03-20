use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use bytes::BytesMut;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;
use tokio::time;
use tracing::{debug, info, warn};

use crate::asdu::Asdu;
use crate::error::RequestError;
use crate::ft12::{
    self, ControlField, LinkFrame, LinkFrameParser, PrimaryFunction, SecondaryFunction,
};
use crate::transport::{connect_with_retry, Connector, PhysLayer};

use super::{task, Client101Config, ClientCommand, ClientHandler, ConnectionState};

pub(super) struct Client101Task<C, H: ClientHandler> {
    connector: C,
    config: Client101Config,
    handler: H,
    param: H::Param,
    commands: mpsc::Receiver<ClientCommand>,
    connected: Arc<AtomicBool>,
}

struct LinkState {
    /// Frame count bit — toggled after each confirmed exchange.
    fcb: bool,
    /// Whether the link has been successfully reset.
    link_reset: bool,
}

impl LinkState {
    fn new() -> Self {
        Self {
            fcb: false,
            link_reset: false,
        }
    }

    /// Toggle FCB after a successful confirmed exchange.
    fn toggle_fcb(&mut self) {
        self.fcb = !self.fcb;
    }
}

enum SessionEnd {
    Shutdown,
    Disconnected,
}

impl<C: Connector, H: ClientHandler> Client101Task<C, H> {
    pub fn new(
        connector: C,
        config: Client101Config,
        handler: H,
        param: H::Param,
        commands: mpsc::Receiver<ClientCommand>,
        connected: Arc<AtomicBool>,
    ) -> Self {
        Self {
            connector,
            config,
            handler,
            param,
            commands,
            connected,
        }
    }

    pub async fn run(self) {
        let Self {
            connector,
            config,
            mut handler,
            mut param,
            mut commands,
            connected,
        } = self;

        loop {
            let phys = connect_with_retry(&connector, &config.retry).await;
            connected.store(true, Ordering::Release);
            handler.on_connection_state(ConnectionState::Connected, &mut param);

            let result =
                run_session(&config, &mut handler, &mut param, &mut commands, phys).await;

            connected.store(false, Ordering::Release);
            handler.on_connection_state(ConnectionState::Disconnected, &mut param);
            task::drain_pending_commands(&mut commands);

            match result {
                Ok(SessionEnd::Shutdown) => return,
                Ok(SessionEnd::Disconnected) => {
                    info!("session ended, reconnecting");
                }
                Err(e) => {
                    warn!(?e, "session error, reconnecting");
                }
            }
        }
    }
}

/// Run a CS101 session: reset link, then poll loop.
async fn run_session<H: ClientHandler>(
    config: &Client101Config,
    handler: &mut H,
    param: &mut H::Param,
    commands: &mut mpsc::Receiver<ClientCommand>,
    phys: PhysLayer,
) -> Result<SessionEnd, RequestError> {
    let (mut reader, mut writer) = tokio::io::split(phys);
    let link = &config.link;
    let mut parser = LinkFrameParser::new(link.link_addr_size());
    let mut state = LinkState::new();

    // Step 1: Reset remote link
    if let Err(e) = reset_link(config, &mut parser, &mut reader, &mut writer, &mut state).await {
        warn!(?e, "link reset failed");
        return Err(e);
    }

    // Step 2: Polling loop
    run_polling_loop(
        config, handler, param, commands, &mut parser, &mut reader, &mut writer, &mut state,
    )
    .await
}

/// Send FC=0 (Reset Link) and wait for ACK.
async fn reset_link<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    config: &Client101Config,
    parser: &mut LinkFrameParser,
    reader: &mut R,
    writer: &mut W,
    state: &mut LinkState,
) -> Result<(), RequestError> {
    let frame = LinkFrame::Fixed {
        control: ControlField::Primary {
            fcb: false,
            fcv: false,
            function: PrimaryFunction::ResetLink,
        },
        address: config.link_address,
    };

    ft12::write_link_frame(writer, &frame, config.link.link_addr_size())
        .await
        .map_err(|e| RequestError::Link(e))?;

    let response = time::timeout(
        config.link.response_timeout(),
        parser.read_frame(reader),
    )
    .await
    .map_err(|_| RequestError::Timeout("link reset response timeout"))?
    .map_err(RequestError::Link)?;

    match &response {
        LinkFrame::Fixed {
            control: ControlField::Secondary { function: SecondaryFunction::Ack, .. },
            ..
        }
        | LinkFrame::SingleAck => {
            info!("link reset confirmed");
            state.link_reset = true;
            state.fcb = true; // FCB starts at true after reset
            Ok(())
        }
        LinkFrame::Fixed {
            control: ControlField::Secondary { function: SecondaryFunction::Nack, .. },
            ..
        } => Err(RequestError::LinkNack),
        _ => Err(RequestError::LinkResetFailed),
    }
}

/// Send a primary frame and wait for the secondary response within timeout.
async fn send_and_receive<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    config: &Client101Config,
    parser: &mut LinkFrameParser,
    reader: &mut R,
    writer: &mut W,
    frame: &LinkFrame,
) -> Result<LinkFrame, RequestError> {
    ft12::write_link_frame(writer, frame, config.link.link_addr_size())
        .await
        .map_err(RequestError::Link)?;

    time::timeout(config.link.response_timeout(), parser.read_frame(reader))
        .await
        .map_err(|_| RequestError::Timeout("link response timeout"))?
        .map_err(RequestError::Link)
}

/// Main polling loop: process commands, poll class 1 and class 2 data.
async fn run_polling_loop<H: ClientHandler, R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    config: &Client101Config,
    handler: &mut H,
    param: &mut H::Param,
    commands: &mut mpsc::Receiver<ClientCommand>,
    parser: &mut LinkFrameParser,
    reader: &mut R,
    writer: &mut W,
    state: &mut LinkState,
) -> Result<SessionEnd, RequestError> {
    loop {
        // Process any pending commands first
        loop {
            match commands.try_recv() {
                Ok(cmd) => match cmd {
                    ClientCommand::StartDt { promise } => {
                        // CS101 has no STARTDT concept — always active after link reset
                        promise.complete(Ok(()));
                    }
                    ClientCommand::StopDt { promise } => {
                        promise.complete(Err(RequestError::NotActive));
                    }
                    ClientCommand::SendAsdu { asdu, promise } => {
                        let result =
                            send_asdu(config, parser, reader, writer, state, &asdu).await;
                        let is_link_err = matches!(&result, Err(RequestError::Link(_)));
                        promise.complete(result);
                        if is_link_err {
                            return Ok(SessionEnd::Disconnected);
                        }
                    }
                    ClientCommand::Shutdown { response } => {
                        let _ = response.send(());
                        return Ok(SessionEnd::Shutdown);
                    }
                },
                Err(_) => break,
            }
        }

        // Poll class 1 data (high priority)
        let (end, acd) = poll_class(
            config, handler, param, parser, reader, writer, state,
            PrimaryFunction::RequestClass1,
        )
        .await?;
        if let Some(end) = end {
            return Ok(end);
        }

        // If ACD was set, repeat class 1 polling
        if acd {
            continue;
        }

        // Poll class 2 data (low priority)
        let (end, _acd) = poll_class(
            config, handler, param, parser, reader, writer, state,
            PrimaryFunction::RequestClass2,
        )
        .await?;
        if let Some(end) = end {
            return Ok(end);
        }

        // Wait for poll_interval or an incoming command
        tokio::select! {
            _ = time::sleep(config.link.poll_interval()) => {}
            cmd = commands.recv() => {
                match cmd {
                    Some(ClientCommand::StartDt { promise }) => {
                        promise.complete(Ok(()));
                    }
                    Some(ClientCommand::StopDt { promise }) => {
                        promise.complete(Err(RequestError::NotActive));
                    }
                    Some(ClientCommand::SendAsdu { asdu, promise }) => {
                        let result =
                            send_asdu(config, parser, reader, writer, state, &asdu).await;
                        promise.complete(result);
                    }
                    Some(ClientCommand::Shutdown { response }) => {
                        let _ = response.send(());
                        return Ok(SessionEnd::Shutdown);
                    }
                    None => return Ok(SessionEnd::Shutdown),
                }
            }
        }
    }
}

/// Poll class 1 or class 2 data from the slave.
///
/// Returns `(Some(SessionEnd), _)` if the session should end, or
/// `(None, acd)` with the ACD flag from the response.
async fn poll_class<H: ClientHandler, R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    config: &Client101Config,
    handler: &mut H,
    param: &mut H::Param,
    parser: &mut LinkFrameParser,
    reader: &mut R,
    writer: &mut W,
    state: &mut LinkState,
    function: PrimaryFunction,
) -> Result<(Option<SessionEnd>, bool), RequestError> {
    let frame = LinkFrame::Fixed {
        control: ControlField::Primary {
            fcb: state.fcb,
            fcv: true,
            function,
        },
        address: config.link_address,
    };

    let response = send_and_receive(config, parser, reader, writer, &frame).await?;
    state.toggle_fcb();

    match response {
        LinkFrame::Variable {
            control: ControlField::Secondary { acd, .. },
            mut data,
            ..
        } => {
            // Decode ASDU from user data
            match Asdu::decode(&mut data, &config.app) {
                Ok(asdu) => handler.on_asdu(&asdu, param),
                Err(e) => warn!(?e, "failed to decode ASDU from class data"),
            }
            Ok((None, acd))
        }
        LinkFrame::Fixed {
            control: ControlField::Secondary { acd, function: SecondaryFunction::NoData, .. },
            ..
        } => {
            debug!("no data available");
            Ok((None, acd))
        }
        LinkFrame::Fixed {
            control: ControlField::Secondary { function: SecondaryFunction::Nack, .. },
            ..
        } => {
            debug!("received NACK for class request");
            Ok((None, false))
        }
        _ => {
            debug!(?response, "unexpected response to class request");
            Ok((None, false))
        }
    }
}

/// Send an ASDU using FC=3 (Send/Confirm) and wait for ACK.
async fn send_asdu<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    config: &Client101Config,
    parser: &mut LinkFrameParser,
    reader: &mut R,
    writer: &mut W,
    state: &mut LinkState,
    asdu: &Asdu,
) -> Result<(), RequestError> {
    let mut payload = BytesMut::new();
    asdu.encode(&mut payload, &config.app)?;

    let frame = LinkFrame::Variable {
        control: ControlField::Primary {
            fcb: state.fcb,
            fcv: true,
            function: PrimaryFunction::SendConfirm,
        },
        address: config.link_address,
        data: payload.freeze(),
    };

    let response = send_and_receive(config, parser, reader, writer, &frame).await?;
    state.toggle_fcb();

    match &response {
        LinkFrame::Fixed {
            control: ControlField::Secondary { function: SecondaryFunction::Ack, .. },
            ..
        }
        | LinkFrame::SingleAck => Ok(()),
        LinkFrame::Fixed {
            control: ControlField::Secondary { function: SecondaryFunction::Nack, .. },
            ..
        } => Err(RequestError::LinkNack),
        _ => {
            let msg = format!("expected ACK for send, got {:?}", response);
            Err(RequestError::UnexpectedResponse(msg))
        }
    }
}

