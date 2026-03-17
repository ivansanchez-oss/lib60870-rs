use std::io;

use bytes::BytesMut;
use tokio::io::{ReadHalf, WriteHalf};
use tokio::sync::mpsc;
use tokio::time::{self, Instant};
use tracing::{debug, info, warn};

use crate::apci::{self, Apdu, UFunction};
use crate::asdu::Asdu;
use crate::transport::{connect_with_retry, Connector, PhysLayer};

use super::{ClientCommand, ClientConfig, ClientHandler, ConnectionState};

pub(super) struct ClientTask<C, H> {
    connector: C,
    config: ClientConfig,
    handler: H,
    commands: mpsc::Receiver<ClientCommand>,
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
        commands: mpsc::Receiver<ClientCommand>,
    ) -> Self {
        Self {
            connector,
            config,
            handler,
            commands,
        }
    }

    pub async fn run(self) {
        let Self {
            connector,
            config,
            mut handler,
            mut commands,
        } = self;

        loop {
            let phys = connect_with_retry(&connector, &config.retry).await;
            handler.on_connection_state(ConnectionState::Connected);

            let result = run_session(&config, &mut handler, &mut commands, phys).await;

            handler.on_connection_state(ConnectionState::Disconnected);

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

async fn run_session<H: ClientHandler>(
    config: &ClientConfig,
    handler: &mut H,
    commands: &mut mpsc::Receiver<ClientCommand>,
    phys: PhysLayer,
) -> io::Result<SessionEnd> {
    let (mut reader, mut writer) = tokio::io::split(phys);

    // Send STARTDT act
    apci::write_apdu(&mut writer, &Apdu::U(UFunction::StartDtAct)).await?;

    // Wait for STARTDT con within t0
    let frame = time::timeout(config.apci.t0, apci::read_apdu(&mut reader))
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "STARTDT con timeout (t0)"))??;

    match frame {
        Apdu::U(UFunction::StartDtCon) => {
            info!("STARTDT confirmed");
        }
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("expected STARTDT con, got {:?}", other),
            ));
        }
    }

    let mut state = SessionState::new();
    run_data_transfer(
        config,
        handler,
        commands,
        &mut reader,
        &mut writer,
        &mut state,
    )
    .await
}

async fn run_data_transfer<H: ClientHandler>(
    config: &ClientConfig,
    handler: &mut H,
    commands: &mut mpsc::Receiver<ClientCommand>,
    reader: &mut ReadHalf<PhysLayer>,
    writer: &mut WriteHalf<PhysLayer>,
    state: &mut SessionState,
) -> io::Result<SessionEnd> {
    let apci_params = &config.apci;

    loop {
        let t3_deadline = state.last_tx.max(state.last_rx) + apci_params.t3;

        tokio::select! {
            // --- Incoming frame ---
            frame_result = apci::read_apdu(reader) => {
                let frame = frame_result?;
                state.last_rx = Instant::now();

                match frame {
                    Apdu::I { send_seq, recv_seq, mut payload } => {
                        if send_seq != state.recv_seq {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                format!(
                                    "sequence error: expected {}, got {}",
                                    state.recv_seq, send_seq
                                ),
                            ));
                        }

                        state.recv_seq = (state.recv_seq + 1) % 32768;
                        process_ack(state, recv_seq);

                        match Asdu::decode(&mut payload, &config.app) {
                            Ok(asdu) => handler.on_asdu(&asdu),
                            Err(e) => warn!(?e, "failed to decode ASDU"),
                        }

                        state.unacked_recv += 1;

                        if state.unacked_recv >= apci_params.w {
                            send_s_frame(writer, state).await?;
                        } else if state.t2_deadline.is_none() {
                            state.t2_deadline = Some(Instant::now() + apci_params.t2);
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
                            // Clear t1 if no unconfirmed I-frames either
                            if state.unconfirmed_count() == 0 {
                                state.t1_deadline = None;
                            }
                        }
                    }

                    Apdu::U(UFunction::StopDtAct) => {
                        apci::write_apdu(writer, &Apdu::U(UFunction::StopDtCon)).await?;
                        return Ok(SessionEnd::Disconnected);
                    }

                    Apdu::U(func) => {
                        debug!(?func, "received unexpected U-frame");
                    }
                }
            }

            // --- User command ---
            cmd = commands.recv() => {
                match cmd {
                    Some(ClientCommand::SendAsdu { asdu, response }) => {
                        let result = send_i_frame(config, writer, state, &asdu).await;
                        let _ = response.send(result);
                    }
                    Some(ClientCommand::Shutdown { response }) => {
                        let _ = apci::write_apdu(writer, &Apdu::U(UFunction::StopDtAct)).await;
                        let _ = response.send(());
                        return Ok(SessionEnd::Shutdown);
                    }
                    None => {
                        // All handles dropped
                        return Ok(SessionEnd::Shutdown);
                    }
                }
            }

            // --- t1 timeout: ack or TESTFR con not received ---
            _ = async {
                match state.t1_deadline {
                    Some(deadline) => time::sleep_until(deadline).await,
                    None => std::future::pending().await,
                }
            } => {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "t1 timeout: no acknowledgment received",
                ));
            }

            // --- t2 timeout: send S-frame ack ---
            _ = async {
                match state.t2_deadline {
                    Some(deadline) => time::sleep_until(deadline).await,
                    None => std::future::pending().await,
                }
            } => {
                send_s_frame(writer, state).await?;
            }

            // --- t3 timeout: send TESTFR ---
            _ = time::sleep_until(t3_deadline) => {
                if !state.testfr_pending {
                    apci::write_apdu(writer, &Apdu::U(UFunction::TestFrAct)).await?;
                    state.last_tx = Instant::now();
                    state.testfr_pending = true;

                    // Expect TESTFR con within t1
                    if state.t1_deadline.is_none() {
                        state.t1_deadline = Some(Instant::now() + apci_params.t1);
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
) -> io::Result<()> {
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

async fn send_i_frame(
    config: &ClientConfig,
    writer: &mut WriteHalf<PhysLayer>,
    state: &mut SessionState,
    asdu: &Asdu,
) -> io::Result<()> {
    if state.unconfirmed_count() >= config.apci.k {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "send window full (k)",
        ));
    }

    let mut payload = BytesMut::new();
    asdu.encode(&mut payload, &config.app)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

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
        state.t1_deadline = Some(Instant::now() + config.apci.t1);
    }

    Ok(())
}
