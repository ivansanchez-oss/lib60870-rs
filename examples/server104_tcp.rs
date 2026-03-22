use std::time::Duration;

use lib60870::asdu::{Asdu, AsduBuilder, InformationObjectAddress};
use lib60870::client::ConnectionState;
use lib60870::info::{InformationObject, SinglePointInformation};
use lib60870::server::{
    AsduResponse, EventClass, Server104, Server104Config, ListenerConfig104, ServerHandler,
    ServerHandle,
};
use lib60870::transport::TcpListenerConfig;
use lib60870::types::{
    ApciParameters, AppLayerParameters, CauseOfTransmission, CommonAddress, QualityDescriptor,
};
use tracing::info;

struct Handler {
    server_handle: Option<ServerHandle>,
}

impl ServerHandler for Handler {
    type Param = ();

    fn on_connection_state(&mut self, state: ConnectionState, _param: &mut ()) {
        info!("Master connection: {:?}", state);
    }

    fn on_asdu(&mut self, asdu: &Asdu, _param: &mut ()) -> AsduResponse {
        info!(
            "Received from master: type={}, cause={:?}, ca={}",
            asdu.header.type_id, asdu.header.cause, asdu.header.common_address,
        );

        // Handle interrogation: enqueue response ASDUs
        if asdu.header.type_id == lib60870::types::TypeId::CIcNa1 {
            if let Some(handle) = &self.server_handle {
                let response = AsduBuilder::new(
                    CauseOfTransmission::InterrogatedByStation,
                    CommonAddress::new(1),
                )
                .add(
                    InformationObjectAddress::from(100u16),
                    InformationObject::SinglePoint(SinglePointInformation::new(
                        true,
                        QualityDescriptor::empty(),
                    )),
                )
                .unwrap()
                .add(
                    InformationObjectAddress::from(101u16),
                    InformationObject::SinglePoint(SinglePointInformation::new(
                        false,
                        QualityDescriptor::empty(),
                    )),
                )
                .unwrap()
                .build()
                .unwrap();

                // Enqueue in a blocking fashion (we're in a sync callback)
                let h = handle.clone();
                tokio::spawn(async move {
                    let _ = h.enqueue_asdu(response, EventClass::Class1).await;
                });
            }
        }

        AsduResponse::Confirm
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

    let addr = "0.0.0.0:2404".parse().unwrap();

    let config = Server104Config {
        apci: ApciParameters::default(),
        app: AppLayerParameters::CS104_DEFAULT,
        max_connections: 1,
    };

    let listener_config = ListenerConfig104::Tcp(TcpListenerConfig::new(addr));

    let server = Server104::new(listener_config, config, Handler { server_handle: None }, ());
    let handle = server.run();

    info!("CS104 server listening on {}", addr);

    // Periodically enqueue spontaneous events
    let event_handle = handle.clone();
    tokio::spawn(async move {
        let mut value = false;
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            value = !value;

            let asdu = AsduBuilder::new(CauseOfTransmission::Spontaneous, CommonAddress::new(1))
                .add(
                    InformationObjectAddress::from(200u16),
                    InformationObject::SinglePoint(SinglePointInformation::new(
                        value,
                        QualityDescriptor::empty(),
                    )),
                )
                .unwrap()
                .build()
                .unwrap();

            if let Err(e) = event_handle.enqueue_asdu(asdu, EventClass::Class1).await {
                info!("Failed to enqueue: {e}");
            }
        }
    });

    tokio::signal::ctrl_c().await.unwrap();
    handle.shutdown().await.unwrap();
}
