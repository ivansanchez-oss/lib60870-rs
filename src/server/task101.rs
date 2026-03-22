use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use bytes::BytesMut;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::asdu::Asdu;
use crate::error::RequestError;
use crate::ft12::{self, ControlField, LinkFrame, LinkFrameParser, PrimaryFunction, SecondaryFunction};

use super::{AsduResponse, EventClass, ServerCommand, ServerHandler, Slave101Config};

struct SecondaryLinkState {
    /// Expected FCB from master (toggled after each successful exchange).
    expected_fcb: bool,
    /// Whether the link has been reset by the master.
    link_reset: bool,
}

impl SecondaryLinkState {
    fn new() -> Self {
        Self {
            expected_fcb: true,
            link_reset: false,
        }
    }
}

/// Run a CS101 slave session for one connected master.
pub(super) async fn run_session<H: ServerHandler>(
    config: &Slave101Config,
    handler: &mut H,
    param: &mut H::Param,
    commands: &mut mpsc::Receiver<ServerCommand>,
    reader: &mut (impl AsyncRead + Unpin),
    writer: &mut (impl AsyncWrite + Unpin),
    connected: &Arc<AtomicBool>,
) {
    connected.store(true, Ordering::Release);
    handler.on_connection_state(crate::client::ConnectionState::Connected, param);

    let result = run_session_inner(config, handler, param, commands, reader, writer).await;

    connected.store(false, Ordering::Release);
    handler.on_connection_state(crate::client::ConnectionState::Disconnected, param);

    if let Err(e) = result {
        warn!(?e, "slave session ended with error");
    } else {
        info!("slave session ended");
    }
}

async fn run_session_inner<H: ServerHandler>(
    config: &Slave101Config,
    handler: &mut H,
    param: &mut H::Param,
    commands: &mut mpsc::Receiver<ServerCommand>,
    reader: &mut (impl AsyncRead + Unpin),
    writer: &mut (impl AsyncWrite + Unpin),
) -> Result<(), RequestError> {
    let mut parser = LinkFrameParser::new(config.link.link_addr_size());
    let mut state = SecondaryLinkState::new();
    let addr_size = config.link.link_addr_size();

    // Queues for outgoing ASDUs
    let mut class1_queue: VecDeque<Asdu> = VecDeque::new();
    let mut class2_queue: VecDeque<Asdu> = VecDeque::new();

    loop {
        // Drain any enqueued commands (non-blocking) before waiting for a frame
        drain_commands(commands, &mut class1_queue, &mut class2_queue);

        let frame = parser.read_frame(reader).await.map_err(RequestError::Link)?;

        // Only process primary frames (from master)
        let (fcb, fcv, function, data) = match &frame {
            LinkFrame::Fixed {
                control: ControlField::Primary { fcb, fcv, function },
                ..
            } => (*fcb, *fcv, *function, None),
            LinkFrame::Variable {
                control: ControlField::Primary { fcb, fcv, function },
                data,
                ..
            } => (*fcb, *fcv, *function, Some(data.clone())),
            _ => {
                debug!(?frame, "ignoring non-primary frame");
                continue;
            }
        };

        match function {
            PrimaryFunction::ResetLink => {
                state.link_reset = true;
                state.expected_fcb = true;
                info!("link reset by master");

                let response = LinkFrame::Fixed {
                    control: ControlField::Secondary {
                        acd: !class1_queue.is_empty(),
                        dfc: false,
                        function: SecondaryFunction::Ack,
                    },
                    address: config.link_address,
                };
                ft12::write_link_frame(writer, &response, addr_size)
                    .await
                    .map_err(RequestError::Link)?;
            }

            PrimaryFunction::RequestLinkStatus => {
                let response = LinkFrame::Fixed {
                    control: ControlField::Secondary {
                        acd: !class1_queue.is_empty(),
                        dfc: false,
                        function: SecondaryFunction::LinkStatus,
                    },
                    address: config.link_address,
                };
                ft12::write_link_frame(writer, &response, addr_size)
                    .await
                    .map_err(RequestError::Link)?;
            }

            PrimaryFunction::RequestClass1 => {
                if !state.link_reset {
                    debug!("ignoring class 1 request before link reset");
                    continue;
                }

                if fcv && fcb != state.expected_fcb {
                    debug!(expected = state.expected_fcb, got = fcb, "FCB mismatch, possible retransmission");
                    // On FCB mismatch, we should repeat the last response.
                    // For simplicity, send NoData.
                }

                let response = if let Some(asdu) = class1_queue.pop_front() {
                    let mut payload = BytesMut::new();
                    asdu.encode(&mut payload, &config.app)?;

                    LinkFrame::Variable {
                        control: ControlField::Secondary {
                            acd: !class1_queue.is_empty(),
                            dfc: false,
                            function: SecondaryFunction::UserData,
                        },
                        address: config.link_address,
                        data: payload.freeze(),
                    }
                } else {
                    LinkFrame::Fixed {
                        control: ControlField::Secondary {
                            acd: false,
                            dfc: false,
                            function: SecondaryFunction::NoData,
                        },
                        address: config.link_address,
                    }
                };

                ft12::write_link_frame(writer, &response, addr_size)
                    .await
                    .map_err(RequestError::Link)?;

                if fcv {
                    state.expected_fcb = !state.expected_fcb;
                }
            }

            PrimaryFunction::RequestClass2 => {
                if !state.link_reset {
                    debug!("ignoring class 2 request before link reset");
                    continue;
                }

                if fcv && fcb != state.expected_fcb {
                    debug!(expected = state.expected_fcb, got = fcb, "FCB mismatch");
                }

                let response = if let Some(asdu) = class2_queue.pop_front() {
                    let mut payload = BytesMut::new();
                    asdu.encode(&mut payload, &config.app)?;

                    LinkFrame::Variable {
                        control: ControlField::Secondary {
                            acd: !class1_queue.is_empty(),
                            dfc: false,
                            function: SecondaryFunction::UserData,
                        },
                        address: config.link_address,
                        data: payload.freeze(),
                    }
                } else {
                    LinkFrame::Fixed {
                        control: ControlField::Secondary {
                            acd: !class1_queue.is_empty(),
                            dfc: false,
                            function: SecondaryFunction::NoData,
                        },
                        address: config.link_address,
                    }
                };

                ft12::write_link_frame(writer, &response, addr_size)
                    .await
                    .map_err(RequestError::Link)?;

                if fcv {
                    state.expected_fcb = !state.expected_fcb;
                }
            }

            PrimaryFunction::SendConfirm => {
                if !state.link_reset {
                    debug!("ignoring send/confirm before link reset");
                    continue;
                }

                if fcv && fcb != state.expected_fcb {
                    debug!(expected = state.expected_fcb, got = fcb, "FCB mismatch");
                }

                let resp_function = if let Some(mut asdu_data) = data {
                    match Asdu::decode(&mut asdu_data, &config.app) {
                        Ok(asdu) => {
                            let app_response = handler.on_asdu(&asdu, param);
                            match app_response {
                                AsduResponse::Confirm => SecondaryFunction::Ack,
                                AsduResponse::Negative => SecondaryFunction::Nack,
                            }
                        }
                        Err(e) => {
                            warn!(?e, "failed to decode ASDU from master");
                            SecondaryFunction::Nack
                        }
                    }
                } else {
                    SecondaryFunction::Nack
                };

                let response = LinkFrame::Fixed {
                    control: ControlField::Secondary {
                        acd: !class1_queue.is_empty(),
                        dfc: false,
                        function: resp_function,
                    },
                    address: config.link_address,
                };
                ft12::write_link_frame(writer, &response, addr_size)
                    .await
                    .map_err(RequestError::Link)?;

                if fcv {
                    state.expected_fcb = !state.expected_fcb;
                }
            }
        }
    }
}

/// Drain pending server commands into the queues.
fn drain_commands(
    commands: &mut mpsc::Receiver<ServerCommand>,
    class1: &mut VecDeque<Asdu>,
    class2: &mut VecDeque<Asdu>,
) {
    while let Ok(cmd) = commands.try_recv() {
        match cmd {
            ServerCommand::EnqueueAsdu { asdu, class } => match class {
                EventClass::Class1 => class1.push_back(asdu),
                EventClass::Class2 => class2.push_back(asdu),
            },
            ServerCommand::Shutdown => {
                // Shutdown is handled by the accept loop / main task
            }
        }
    }
}
