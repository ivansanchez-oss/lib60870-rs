use std::time::Duration;

use lib60870::asdu::{Asdu, AsduBuilder, InformationObjectAddress};
use lib60870::client::ConnectionState;
use lib60870::ft12::LinkAddress;
use lib60870::info::{InformationObject, SinglePointInformation};
use lib60870::server::{
    AsduResponse, EventClass, ListenerConfig101, ServerHandler, Slave101, Slave101Config,
};
use lib60870::transport::SerialOverTcpListenerConfig;
use lib60870::types::{
    AppLayerParameters, CauseOfTransmission, CommonAddress, LinkLayerParameters, QualityDescriptor,
};
use tracing::info;

struct Handler;

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
        AsduResponse::Confirm
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

    let addr = "0.0.0.0:2404".parse().unwrap();

    let config = Slave101Config {
        link: LinkLayerParameters::builder()
            .link_addr_size(1)
            .response_timeout(Duration::from_millis(500))
            .poll_interval(Duration::from_millis(1000))
            .build()
            .expect("Invalid link parameters"),
        app: AppLayerParameters::CS101_DEFAULT,
        link_address: LinkAddress(1),
    };

    let listener_config =
        ListenerConfig101::SerialOverTcp(SerialOverTcpListenerConfig::new(addr));

    let server = Slave101::new(listener_config, config, Handler, ());
    let handle = server.run();

    info!("CS101 slave (serial-over-TCP) listening on {}", addr);

    // Periodically enqueue spontaneous events
    let event_handle = handle.clone();
    tokio::spawn(async move {
        let mut value = false;
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            value = !value;

            let asdu = AsduBuilder::new(CauseOfTransmission::Spontaneous, CommonAddress::new(1))
                .add(
                    InformationObjectAddress::from(100u16),
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
