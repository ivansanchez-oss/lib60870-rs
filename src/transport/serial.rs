use std::io;

use tokio_serial::SerialStream;
use tracing::info;

use super::config::{DataBits, Parity, SerialConfig, StopBits};
use super::{Connector, PhysLayer};

/// Connector that opens a local serial port.
#[derive(Debug, Clone)]
pub struct SerialConnector {
    pub config: SerialConfig,
}

impl SerialConnector {
    pub fn new(config: SerialConfig) -> Self {
        Self { config }
    }
}

impl Connector for SerialConnector {
    async fn connect(&self) -> io::Result<PhysLayer> {
        let builder = tokio_serial::new(&self.config.path, self.config.baud_rate)
            .data_bits(convert_data_bits(self.config.data_bits))
            .parity(convert_parity(self.config.parity))
            .stop_bits(convert_stop_bits(self.config.stop_bits));

        let stream = SerialStream::open(&builder)?;
        info!(path = %self.config.path, baud = self.config.baud_rate, "serial port opened");
        Ok(PhysLayer::new(stream))
    }
}

fn convert_data_bits(bits: DataBits) -> tokio_serial::DataBits {
    match bits {
        DataBits::Five => tokio_serial::DataBits::Five,
        DataBits::Six => tokio_serial::DataBits::Six,
        DataBits::Seven => tokio_serial::DataBits::Seven,
        DataBits::Eight => tokio_serial::DataBits::Eight,
    }
}

fn convert_parity(parity: Parity) -> tokio_serial::Parity {
    match parity {
        Parity::None => tokio_serial::Parity::None,
        Parity::Odd => tokio_serial::Parity::Odd,
        Parity::Even => tokio_serial::Parity::Even,
    }
}

fn convert_stop_bits(bits: StopBits) -> tokio_serial::StopBits {
    match bits {
        StopBits::One => tokio_serial::StopBits::One,
        StopBits::Two => tokio_serial::StopBits::Two,
    }
}
