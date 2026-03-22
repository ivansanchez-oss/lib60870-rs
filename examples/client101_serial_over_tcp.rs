use std::time::Duration;

use lib60870::asdu::Asdu;
use lib60870::client::{
    Client101, Client101Config, ClientHandler, ConnectionState, TransportConfig101,
};
use lib60870::ft12::LinkAddress;
use lib60870::transport::{RetryStrategy, SerialOverTcpConfig};
use lib60870::types::{AppLayerParameters, CommonAddress, LinkLayerParameters};
use tracing::{error, info};

struct Handler;

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

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

    let addr = "127.0.0.1:2404".parse().unwrap();

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

    let transport = TransportConfig101::SerialOverTcp(SerialOverTcpConfig::new(addr));

    let handle = Client101::new(transport, config, Handler, ()).run();

    // Wait for connection
    while !handle.is_connected() {
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    // CS101 is always active after link reset — no STARTDT needed.
    // Send a station interrogation.
    if let Err(e) = handle.interrogation(CommonAddress::new(1), 20).await {
        error!("Interrogation error: {e}");
    }

    tokio::signal::ctrl_c().await.unwrap();
    handle.shutdown().await.unwrap();
}
