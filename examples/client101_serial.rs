/// CS101 client over native serial port.
///
/// Requires the `serial` feature: cargo run --example client101_serial --features serial
#[cfg(feature = "serial")]
use std::time::Duration;

#[cfg(feature = "serial")]
use lib60870::asdu::Asdu;
#[cfg(feature = "serial")]
use lib60870::client::{
    Client101, Client101Config, ClientHandler, ConnectionState, TransportConfig101,
};
#[cfg(feature = "serial")]
use lib60870::ft12::LinkAddress;
#[cfg(feature = "serial")]
use lib60870::transport::{RetryStrategy, SerialConfig, DataBits, Parity, StopBits};
#[cfg(feature = "serial")]
use lib60870::types::{AppLayerParameters, CommonAddress, LinkLayerParameters};
#[cfg(feature = "serial")]
use tracing::{error, info};

#[cfg(feature = "serial")]
struct Handler;

#[cfg(feature = "serial")]
impl ClientHandler for Handler {
    type Param = ();

    fn on_connection_state(&mut self, state: ConnectionState, _param: &mut ()) {
        info!("Connection state: {:?}", state);
    }

    fn on_asdu(&mut self, asdu: &Asdu, _param: &mut ()) {
        info!(
            "ASDU: type={}, cause={:?}, ca={}, objects={}",
            asdu.header.type_id,
            asdu.header.cause,
            asdu.header.common_address,
            asdu.objects.len(),
        );
        for obj in &asdu.objects {
            info!("  IOA {}: {:?}", obj.address.value(), obj.value);
        }
    }
}

#[cfg(feature = "serial")]
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

    let config = Client101Config {
        link: LinkLayerParameters::builder()
            .link_addr_size(1)
            .response_timeout(Duration::from_millis(500))
            .poll_interval(Duration::from_millis(1000))
            .build()
            .expect("Invalid link parameters"),
        app: AppLayerParameters::CS101_DEFAULT,
        retry: RetryStrategy {
            min_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(30),
        },
        link_address: LinkAddress(1),
    };

    let transport = TransportConfig101::Serial(SerialConfig {
        path: "/dev/ttyUSB0".to_string(),
        baud_rate: 9600,
        data_bits: DataBits::Eight,
        parity: Parity::Even,
        stop_bits: StopBits::One,
    });

    let handle = Client101::new(transport, config, Handler, ()).run();

    while !handle.is_connected() {
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    if let Err(e) = handle.interrogation(CommonAddress::new(1), 20).await {
        error!("Interrogation error: {e}");
    }

    tokio::signal::ctrl_c().await.unwrap();
    handle.shutdown().await.unwrap();
}

#[cfg(not(feature = "serial"))]
fn main() {
    eprintln!("This example requires the 'serial' feature.");
    eprintln!("Run with: cargo run --example client101_serial --features serial");
}
