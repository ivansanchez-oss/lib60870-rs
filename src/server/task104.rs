use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use bytes::BytesMut;
use tokio::io::WriteHalf;
use tokio::sync::mpsc;
use tokio::time::{self, Instant};
use tracing::{debug, info, warn};

use crate::apci::{self, Apdu, FrameReader, UFunction};
use crate::asdu::Asdu;
use crate::error::RequestError;
use crate::transport::PhysLayer;
use super::{AsduResponse, EventClass, ServerCommand, ServerHandler, Server104Config};

struct SessionState {
    send_seq: u16,
    recv_seq: u16,
    ack_seq: u16,
    unacked_recv: u16,
    testfr_pending: bool,
    data_transfer_active: bool,

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
            data_transfer_active: false,
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

/// Run a single CS104 server session for one connected master.
pub(super) async fn run_session<H: ServerHandler>(
    config: &Server104Config,
    handler: &mut H,
    param: &mut H::Param,
    commands: &mut mpsc::Receiver<ServerCommand>,
    phys: PhysLayer,
    connected: &Arc<AtomicBool>,
) {
    connected.store(true, Ordering::Release);
    handler.on_connection_state(crate::client::ConnectionState::Connected, param);

    let result = run_session_inner(config, handler, param, commands, phys).await;

    connected.store(false, Ordering::Release);
    handler.on_connection_state(crate::client::ConnectionState::Disconnected, param);

    if let Err(e) = result {
        warn!(?e, "server session ended with error");
    } else {
        info!("server session ended");
    }
}

async fn run_session_inner<H: ServerHandler>(
    config: &Server104Config,
    handler: &mut H,
    param: &mut H::Param,
    commands: &mut mpsc::Receiver<ServerCommand>,
    phys: PhysLayer,
) -> Result<(), RequestError> {
    let (reader, mut writer) = tokio::io::split(phys);
    let mut frame_reader = FrameReader::new();
    let mut reader = reader;
    let mut state = SessionState::new();

    // Queues for outgoing ASDUs
    let mut class1_queue: VecDeque<Asdu> = VecDeque::new();
    let mut class2_queue: VecDeque<Asdu> = VecDeque::new();

    let apci_params = &config.apci;

    loop {
        let t3_deadline = state.last_tx.max(state.last_rx) + apci_params.t3();

        tokio::select! {
            // --- Incoming frame from master ---
            frame_result = frame_reader.read_frame(&mut reader) => {
                let frame = frame_result?;
                state.last_rx = Instant::now();

                match frame {
                    Apdu::U(UFunction::StartDtAct) => {
                        apci::write_apdu(&mut writer, &Apdu::U(UFunction::StartDtCon)).await?;
                        state.last_tx = Instant::now();
                        state.data_transfer_active = true;
                        info!("STARTDT confirmed, data transfer active");
                    }

                    Apdu::U(UFunction::StopDtAct) => {
                        apci::write_apdu(&mut writer, &Apdu::U(UFunction::StopDtCon)).await?;
                        state.last_tx = Instant::now();
                        state.data_transfer_active = false;
                        info!("STOPDT confirmed, data transfer paused");
                    }

                    Apdu::U(UFunction::TestFrAct) => {
                        apci::write_apdu(&mut writer, &Apdu::U(UFunction::TestFrCon)).await?;
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

                    Apdu::I { send_seq, recv_seq, mut payload } => {
                        if !state.data_transfer_active {
                            debug!("received I-frame while data transfer not active, ignoring");
                            continue;
                        }

                        if send_seq != state.recv_seq {
                            return Err(RequestError::SequenceError {
                                expected: state.recv_seq,
                                got: send_seq,
                            });
                        }

                        state.recv_seq = (state.recv_seq + 1) % 32768;
                        process_ack(&mut state, recv_seq);

                        match Asdu::decode(&mut payload, &config.app) {
                            Ok(asdu) => {
                                let response = handler.on_asdu(&asdu, param);
                                handle_asdu_response(
                                    config,
                                    &mut writer,
                                    &mut state,
                                    &asdu,
                                    response,
                                ).await?;
                            }
                            Err(e) => warn!(?e, "failed to decode ASDU from master"),
                        }

                        state.unacked_recv += 1;

                        if state.unacked_recv >= apci_params.w() {
                            send_s_frame(&mut writer, &mut state).await?;
                        } else if state.t2_deadline.is_none() {
                            state.t2_deadline = Some(Instant::now() + apci_params.t2());
                        }
                    }

                    Apdu::S { recv_seq } => {
                        process_ack(&mut state, recv_seq);
                    }

                    Apdu::U(func) => {
                        debug!(?func, "received unexpected U-frame");
                    }
                }
            }

            // --- Enqueued ASDU from user / shutdown ---
            cmd = commands.recv() => {
                match cmd {
                    Some(ServerCommand::EnqueueAsdu { asdu, class }) => {
                        match class {
                            EventClass::Class1 => class1_queue.push_back(asdu),
                            EventClass::Class2 => class2_queue.push_back(asdu),
                        }
                    }
                    Some(ServerCommand::Shutdown) | None => {
                        return Ok(());
                    }
                }
            }

            // --- Send queued ASDUs as I-frames (class 1 has priority) ---
            _ = async {
                if state.data_transfer_active
                    && state.unconfirmed_count() < apci_params.k()
                    && (!class1_queue.is_empty() || !class2_queue.is_empty())
                {
                    // Ready immediately
                } else {
                    std::future::pending::<()>().await;
                }
            } => {
                // Drain class 1 first, then class 2
                let asdu = class1_queue.pop_front()
                    .or_else(|| class2_queue.pop_front());

                if let Some(asdu) = asdu {
                    send_i_frame(config, &mut writer, &mut state, &asdu).await?;
                }
            }

            // --- T1 timeout ---
            _ = async {
                match state.t1_deadline {
                    Some(deadline) => time::sleep_until(deadline).await,
                    None => std::future::pending().await,
                }
            } => {
                return Err(RequestError::Timeout("t1 timeout: no acknowledgment received"));
            }

            // --- T2 timeout ---
            _ = async {
                match state.t2_deadline {
                    Some(deadline) => time::sleep_until(deadline).await,
                    None => std::future::pending().await,
                }
            } => {
                send_s_frame(&mut writer, &mut state).await?;
            }

            // --- T3 timeout ---
            _ = time::sleep_until(t3_deadline) => {
                if !state.testfr_pending {
                    apci::write_apdu(&mut writer, &Apdu::U(UFunction::TestFrAct)).await?;
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

/// Build and send an activation confirmation for a received ASDU.
async fn handle_asdu_response(
    config: &Server104Config,
    writer: &mut WriteHalf<PhysLayer>,
    state: &mut SessionState,
    original: &Asdu,
    response: AsduResponse,
) -> Result<(), RequestError> {
    use crate::types::CauseOfTransmission;

    // Only send confirmation for Activation requests
    let cot = original.header.cause;
    if cot != CauseOfTransmission::Activation && cot != CauseOfTransmission::Deactivation {
        return Ok(());
    }

    let confirm_cot = if cot == CauseOfTransmission::Activation {
        CauseOfTransmission::ActivationCon
    } else {
        CauseOfTransmission::DeactivationCon
    };

    let is_negative = matches!(response, AsduResponse::Negative);

    let confirm = Asdu {
        header: crate::asdu::AsduHeader {
            type_id: original.header.type_id,
            is_sequence: original.header.is_sequence,
            num_objects: original.header.num_objects,
            cause: confirm_cot,
            is_test: original.header.is_test,
            is_negative,
            originator_address: original.header.originator_address,
            common_address: original.header.common_address,
        },
        objects: original.objects.clone(),
    };

    send_i_frame(config, writer, state, &confirm).await
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

async fn send_i_frame(
    config: &Server104Config,
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

    if state.t1_deadline.is_none() {
        state.t1_deadline = Some(Instant::now() + config.apci.t1());
    }

    Ok(())
}
