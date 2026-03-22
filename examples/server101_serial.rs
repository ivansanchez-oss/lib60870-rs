/// CS101 slave over native serial port.
///
/// Requires the `serial` feature: cargo run --example server101_serial --features serial
#[cfg(feature = "serial")]
use std::time::Duration;

#[cfg(feature = "serial")]
use lib60870::asdu::{Asdu, AsduBuilder, InformationObjectAddress};
#[cfg(feature = "serial")]
use lib60870::client::ConnectionState;
#[cfg(feature = "serial")]
use lib60870::ft12::LinkAddress;
#[cfg(feature = "serial")]
use lib60870::info::{InformationObject, SinglePointInformation};
#[cfg(feature = "serial")]
use lib60870::server::{
    AsduResponse, EventClass, ListenerConfig101, ServerHandler, Slave101, Slave101Config,
};
#[cfg(feature = "serial")]
use lib60870::transport::{SerialConfig, DataBits, Parity, StopBits};
#[cfg(feature = "serial")]
use lib60870::types::{
    AppLayerParameters, CauseOfTransmission, CommonAddress, LinkLayerParameters, QualityDescriptor,
};
#[cfg(feature = "serial")]
use tracing::info;

#[cfg(feature = "serial")]
struct Handler;

#[cfg(feature = "serial")]
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

#[cfg(feature = "serial")]
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

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

    let transport = ListenerConfig101::Serial(SerialConfig {
        path: "/dev/ttyUSB0".to_string(),
        baud_rate: 9600,
        data_bits: DataBits::Eight,
        parity: Parity::Even,
        stop_bits: StopBits::One,
    });

    let server = Slave101::new(transport, config, Handler, ());
    let handle = server.run();

    info!("CS101 slave (serial) running on /dev/ttyUSB0");

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

#[cfg(not(feature = "serial"))]
fn main() {
    eprintln!("This example requires the 'serial' feature.");
    eprintln!("Run with: cargo run --example server101_serial --features serial");
}
