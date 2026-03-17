use std::time::Duration;

use lib60870::asdu::Asdu;
use lib60870::client::{Client104, ClientConfig, ClientHandler, ConnectionState, TransportConfig};
use lib60870::transport::{RetryStrategy, TcpConfig};
use lib60870::types::{ApciParameters, AppLayerParameters};
use tracing::{error, info};

struct CustomHandler;

impl ClientHandler for CustomHandler {
    fn on_connection_state(&mut self, state: ConnectionState) {
        info!("Connection state: {:?}", state);
    }

    fn on_asdu(&mut self, asdu: &Asdu) {
        info!(
            "Received ASDU: type={}, cause={:?}, ca={}, objects={}",
            asdu.header.type_id,
            asdu.header.cause,
            asdu.header.common_address,
            asdu.objects.len(),
        );

        for obj in &asdu.objects {
            info!("  IOA {}: {:?}", obj.address.value(), obj.object);
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

    let addr = "192.168.101.3:2404".parse().unwrap();

    let config = ClientConfig {
        apci: ApciParameters {
            k: 12,
            w: 8,
            t0: Duration::from_secs(10),
            t1: Duration::from_secs(15),
            t2: Duration::from_secs(10),
            t3: Duration::from_secs(20),
        },

        app: AppLayerParameters {
            size_of_cot: 2,
            size_of_ca: 2,
            size_of_ioa: 3,
            max_asdu_length: 249,
            originator_address: 0,
        },
        retry: RetryStrategy {
            min_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(30),
        },
    };

    let transport_config = TransportConfig::Tcp(TcpConfig {
        remote_addr: addr,
        connect_timeout: Duration::from_secs(10),
    });

    let handle = Client104::new(transport_config, config, CustomHandler).run();

    // Send a station interrogation to common address 1
    if let Err(e) = handle.interrogation(1, 20).await {
        error!("Interrogation error: {e}");
    }

    // Keep running until Ctrl+C
    tokio::signal::ctrl_c().await.unwrap();
    handle.shutdown().await.unwrap();
}
