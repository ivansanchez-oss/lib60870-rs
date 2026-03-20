use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use bytes::BytesMut;
use tokio::io::{ReadHalf, WriteHalf};
use tokio::sync::mpsc;
use tokio::time::{self, Instant};
use tracing::{debug, info, warn};

use crate::apci::{self, Apdu, FrameReader, UFunction};
use crate::asdu::Asdu;
use crate::error::RequestError;
use crate::transport::{connect_with_retry, Connector, PhysLayer};

use super::{ClientCommand, ClientConfig, ClientHandler, ConnectionState};

pub(super) struct ClientTask<C, H: ClientHandler> {
    connector: C,
    config: ClientConfig,
    handler: H,
    param: H::Param,
    commands: mpsc::Receiver<ClientCommand>,
    connected: Arc<AtomicBool>,
}

struct SessionState {
    /// Next send sequence number V(S).
    send_seq: u16,
    /// Next expected receive sequence number V(R).
    recv_seq: u16,
    /// Last N(R) acknowledged by remote — they confirmed up to this.
    ack_seq: u16,
    /// Number of received I-frames we haven't S-frame-acknowledged.
    unacked_recv: u16,
    /// Whether we sent TESTFR act and are waiting for con.
    testfr_pending: bool,

    // Timer bookkeeping
    last_tx: Instant,
    last_rx: Instant,
    t1_deadline: Option<Instant>,
    t2_deadline: Option<Instant>,
}

impl SessionState {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            send_seq: 0,
            recv_seq: 0,
            ack_seq: 0,
            unacked_recv: 0,
            testfr_pending: false,
            last_tx: now,
            last_rx: now,
            t1_deadline: None,
            t2_deadline: None,
        }
    }

    fn unconfirmed_count(&self) -> u16 {
        self.send_seq.wrapping_sub(self.ack_seq) % 32768
    }
}

enum SessionEnd {
    Shutdown,
    Disconnected,
}

impl<C: Connector, H: ClientHandler> ClientTask<C, H> {
    pub fn new(
        connector: C,
        config: ClientConfig,
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

            let result = run_session(&config, &mut handler, &mut param, &mut commands, phys).await;

            connected.store(false, Ordering::Release);
            handler.on_connection_state(ConnectionState::Disconnected, &mut param);
            drain_pending_commands(&mut commands);

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

/// Run the session: wait for STARTDT, then data transfer, loop on STOPDT.
async fn run_session<H: ClientHandler>(
    config: &ClientConfig,
    handler: &mut H,
    param: &mut H::Param,
    commands: &mut mpsc::Receiver<ClientCommand>,
    phys: PhysLayer,
) -> Result<SessionEnd, RequestError> {
    let (reader, mut writer) = tokio::io::split(phys);
    let mut frame_reader = FrameReader::new();
    let mut reader = reader;

    loop {
        // Wait for user to send STARTDT
        if let Some(end) = wait_for_start_dt(config, commands, &mut frame_reader, &mut reader, &mut writer).await? {
            return Ok(end);
        }

        // Data transfer until STOPDT or error
        let mut state = SessionState::new();
        match run_data_transfer(
            config,
            handler,
            param,
            commands,
            &mut frame_reader,
            &mut reader,
            &mut writer,
            &mut state,
        )
        .await?
        {
            DataTransferEnd::Shutdown => return Ok(SessionEnd::Shutdown),
            DataTransferEnd::Disconnected => return Ok(SessionEnd::Disconnected),
            DataTransferEnd::Stopped => {
                info!("data transfer stopped, waiting for STARTDT");
                continue;
            }
        }
    }
}

/// Returned by `run_data_transfer` to indicate why it exited.
enum DataTransferEnd {
    Shutdown,
    Disconnected,
    /// STOPDT completed — session returns to the stopped state.
    Stopped,
}

/// Wait for the user to send a STARTDT command via the handle.
///
/// Returns `Ok(None)` when STARTDT succeeds (proceed to data transfer),
/// or `Ok(Some(SessionEnd))` to exit the session.
///
/// Rejects SendAsdu/StopDt commands with appropriate errors. Responds to
/// TESTFR from the server to keep the connection alive.
async fn wait_for_start_dt(
    config: &ClientConfig,
    commands: &mut mpsc::Receiver<ClientCommand>,
    frame_reader: &mut FrameReader,
    reader: &mut ReadHalf<PhysLayer>,
    writer: &mut WriteHalf<PhysLayer>,
) -> Result<Option<SessionEnd>, RequestError> {
    loop {
        tokio::select! {
            frame_result = frame_reader.read_frame(reader) => {
                let frame = frame_result?;
                match frame {
                    Apdu::U(UFunction::TestFrAct) => {
                        apci::write_apdu(writer, &Apdu::U(UFunction::TestFrCon)).await?;
                    }
                    Apdu::U(UFunction::StopDtAct) => {
                        apci::write_apdu(writer, &Apdu::U(UFunction::StopDtCon)).await?;
                        return Ok(Some(SessionEnd::Disconnected));
                    }
                    other => {
                        debug!(?other, "received frame while waiting for STARTDT command");
                    }
                }
            }

            cmd = commands.recv() => {
                match cmd {
                    Some(ClientCommand::StartDt { promise }) => {
                        apci::write_apdu(writer, &Apdu::U(UFunction::StartDtAct)).await?;

                        let result = time::timeout(config.apci.t0(), frame_reader.read_frame(reader))
                            .await
                            .map_err(|_| RequestError::Timeout("STARTDT con timeout (t0)"))?;

                        match result? {
                            Apdu::U(UFunction::StartDtCon) => {
                                info!("STARTDT confirmed");
                                let _ = promise.complete(Ok(()));
                                return Ok(None);
                            }
                            other => {
                                let msg = format!("expected STARTDT con, got {:?}", other);
                                let _ = promise.complete(Err(RequestError::UnexpectedResponse(msg.clone())));
                                return Err(RequestError::UnexpectedResponse(msg));
                            }
                        }
                    }
                    Some(ClientCommand::StopDt { promise }) => {
                        let _ = promise.complete(Err(RequestError::NotActive));
                    }
                    Some(ClientCommand::SendAsdu { promise, .. }) => {
                        let _ = promise.complete(Err(RequestError::NotConnected));
                    }
                    Some(ClientCommand::Shutdown { response }) => {
                        let _ = response.send(());
                        return Ok(Some(SessionEnd::Shutdown));
                    }
                    None => return Ok(Some(SessionEnd::Shutdown)),
                }
            }
        }
    }
}

async fn run_data_transfer<H: ClientHandler>(
    config: &ClientConfig,
    handler: &mut H,
    param: &mut H::Param,
    commands: &mut mpsc::Receiver<ClientCommand>,
    frame_reader: &mut FrameReader,
    reader: &mut ReadHalf<PhysLayer>,
    writer: &mut WriteHalf<PhysLayer>,
    state: &mut SessionState,
) -> Result<DataTransferEnd, RequestError> {
    let apci_params = &config.apci;

    loop {
        let t3_deadline = state.last_tx.max(state.last_rx) + apci_params.t3();

        tokio::select! {
            // --- Incoming frame ---
            frame_result = frame_reader.read_frame(reader) => {
                let frame = frame_result?;
                state.last_rx = Instant::now();

                match frame {
                    Apdu::I { send_seq, recv_seq, mut payload } => {
                        if send_seq != state.recv_seq {
                            return Err(RequestError::SequenceError {
                                expected: state.recv_seq,
                                got: send_seq,
                            });
                        }

                        state.recv_seq = (state.recv_seq + 1) % 32768;
                        process_ack(state, recv_seq);

                        match Asdu::decode(&mut payload, &config.app) {
                            Ok(asdu) => handler.on_asdu(&asdu, param),
                            Err(e) => warn!(?e, "failed to decode ASDU"),
                        }

                        state.unacked_recv += 1;

                        if state.unacked_recv >= apci_params.w() {
                            send_s_frame(writer, state).await?;
                        } else if state.t2_deadline.is_none() {
                            state.t2_deadline = Some(Instant::now() + apci_params.t2());
                        }
                    }

                    Apdu::S { recv_seq } => {
                        process_ack(state, recv_seq);
                    }

                    Apdu::U(UFunction::TestFrAct) => {
                        apci::write_apdu(writer, &Apdu::U(UFunction::TestFrCon)).await?;
                        state.last_tx = Instant::now();
                    }

                    Apdu::U(UFunction::TestFrCon) => {
                        if state.testfr_pending {
                            state.testfr_pending = false;
                            if state.unconfirmed_count() == 0 {
                                state.t1_deadline = None;
                            }
                        }
                    }

                    Apdu::U(UFunction::StopDtAct) => {
                        apci::write_apdu(writer, &Apdu::U(UFunction::StopDtCon)).await?;
                        return Ok(DataTransferEnd::Stopped);
                    }

                    Apdu::U(func) => {
                        debug!(?func, "received unexpected U-frame");
                    }
                }
            }

            // --- User command ---
            cmd = commands.recv() => {
                match cmd {
                    Some(ClientCommand::StartDt { promise }) => {
                        let _ = promise.complete(Err(RequestError::AlreadyActive));
                    }
                    Some(ClientCommand::StopDt { promise }) => {
                        let result = apci::write_apdu(writer, &Apdu::U(UFunction::StopDtAct)).await;
                        if let Err(e) = result {
                            let _ = promise.complete(Err(e.into()));
                            return Ok(DataTransferEnd::Disconnected);
                        }
                        // Wait for STOPDT con within t0
                        match time::timeout(config.apci.t0(), frame_reader.read_frame(reader)).await {
                            Ok(Ok(Apdu::U(UFunction::StopDtCon))) => {
                                info!("STOPDT confirmed");
                                let _ = promise.complete(Ok(()));
                                return Ok(DataTransferEnd::Stopped);
                            }
                            Ok(Ok(other)) => {
                                let msg = format!("expected STOPDT con, got {:?}", other);
                                let _ = promise.complete(Err(RequestError::UnexpectedResponse(msg.clone())));
                                return Err(RequestError::UnexpectedResponse(msg));
                            }
                            Ok(Err(e)) => {
                                let err = RequestError::from(e);
                                let _ = promise.complete(Err(RequestError::NotConnected));
                                return Err(err);
                            }
                            Err(_) => {
                                let _ = promise.complete(Err(RequestError::Timeout("STOPDT con timeout (t0)")));
                                return Err(RequestError::Timeout("STOPDT con timeout (t0)"));
                            }
                        }
                    }
                    Some(ClientCommand::SendAsdu { asdu, promise }) => {
                        let result = send_i_frame(config, writer, state, &asdu).await;
                        let _ = promise.complete(result);
                    }
                    Some(ClientCommand::Shutdown { response }) => {
                        let _ = apci::write_apdu(writer, &Apdu::U(UFunction::StopDtAct)).await;
                        let _ = response.send(());
                        return Ok(DataTransferEnd::Shutdown);
                    }
                    None => {
                        return Ok(DataTransferEnd::Shutdown);
                    }
                }
            }

            // --- t1 timeout ---
            _ = async {
                match state.t1_deadline {
                    Some(deadline) => time::sleep_until(deadline).await,
                    None => std::future::pending().await,
                }
            } => {
                return Err(RequestError::Timeout("t1 timeout: no acknowledgment received"));
            }

            // --- t2 timeout ---
            _ = async {
                match state.t2_deadline {
                    Some(deadline) => time::sleep_until(deadline).await,
                    None => std::future::pending().await,
                }
            } => {
                send_s_frame(writer, state).await?;
            }

            // --- t3 timeout ---
            _ = time::sleep_until(t3_deadline) => {
                if !state.testfr_pending {
                    apci::write_apdu(writer, &Apdu::U(UFunction::TestFrAct)).await?;
                    state.last_tx = Instant::now();
                    state.testfr_pending = true;

                    if state.t1_deadline.is_none() {
                        state.t1_deadline = Some(Instant::now() + apci_params.t1());
                    }
                }
            }
        }
    }
}

fn process_ack(state: &mut SessionState, recv_seq: u16) {
    state.ack_seq = recv_seq;
    if state.unconfirmed_count() == 0 && !state.testfr_pending {
        state.t1_deadline = None;
    }
}

async fn send_s_frame(
    writer: &mut WriteHalf<PhysLayer>,
    state: &mut SessionState,
) -> Result<(), RequestError> {
    apci::write_apdu(
        writer,
        &Apdu::S {
            recv_seq: state.recv_seq,
        },
    )
    .await?;
    state.last_tx = Instant::now();
    state.unacked_recv = 0;
    state.t2_deadline = None;
    Ok(())
}

/// Drain all pending commands from the channel, responding with errors.
///
/// Called after a session ends so that callers waiting on `send_asdu`
/// get an immediate error instead of hanging until the next reconnect.
pub(super) fn drain_pending_commands(commands: &mut mpsc::Receiver<ClientCommand>) {
    while let Ok(cmd) = commands.try_recv() {
        match cmd {
            ClientCommand::StartDt { promise } | ClientCommand::StopDt { promise } => {
                let _ = promise.complete(Err(RequestError::NotConnected));
            }
            ClientCommand::SendAsdu { promise, .. } => {
                let _ = promise.complete(Err(RequestError::NotConnected));
            }
            ClientCommand::Shutdown { response } => {
                let _ = response.send(());
            }
        }
    }
}

async fn send_i_frame(
    config: &ClientConfig,
    writer: &mut WriteHalf<PhysLayer>,
    state: &mut SessionState,
    asdu: &Asdu,
) -> Result<(), RequestError> {
    if state.unconfirmed_count() >= config.apci.k() {
        return Err(RequestError::SendWindowFull);
    }

    let mut payload = BytesMut::new();
    asdu.encode(&mut payload, &config.app)?;

    let apdu = Apdu::I {
        send_seq: state.send_seq,
        recv_seq: state.recv_seq,
        payload: payload.freeze(),
    };
    apci::write_apdu(writer, &apdu).await?;

    state.send_seq = (state.send_seq + 1) % 32768;
    state.last_tx = Instant::now();

    // Sending an I-frame also acknowledges received frames
    state.unacked_recv = 0;
    state.t2_deadline = None;

    // Set t1 if this is the first unconfirmed
    if state.t1_deadline.is_none() {
        state.t1_deadline = Some(Instant::now() + config.apci.t1());
    }

    Ok(())
}
