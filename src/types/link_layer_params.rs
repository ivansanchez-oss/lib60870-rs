use std::time::Duration;

use crate::error::ConfigError;

/// Link layer parameters for IEC 60870-5-101 (FT 1.2).
///
/// Controls link address size, response timeouts, and polling intervals.
/// Use [`LinkLayerParametersBuilder`] for custom configurations with validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkLayerParameters {
    link_addr_size: u8,
    response_timeout: Duration,
    poll_interval: Duration,
}

impl Default for LinkLayerParameters {
    fn default() -> Self {
        Self {
            link_addr_size: 1,
            response_timeout: Duration::from_millis(500),
            poll_interval: Duration::from_millis(1000),
        }
    }
}

impl LinkLayerParameters {
    /// Create a builder initialized with default values.
    pub fn builder() -> LinkLayerParametersBuilder {
        LinkLayerParametersBuilder::default()
    }

    /// Size of the link address field (1 or 2 bytes).
    pub fn link_addr_size(&self) -> u8 {
        self.link_addr_size
    }

    /// Timeout waiting for a response from the slave station.
    pub fn response_timeout(&self) -> Duration {
        self.response_timeout
    }

    /// Interval between polling cycles when no data is pending.
    pub fn poll_interval(&self) -> Duration {
        self.poll_interval
    }
}

/// Builder for [`LinkLayerParameters`] with constraint validation.
#[derive(Debug, Clone)]
pub struct LinkLayerParametersBuilder {
    link_addr_size: u8,
    response_timeout: Duration,
    poll_interval: Duration,
}

impl Default for LinkLayerParametersBuilder {
    fn default() -> Self {
        let defaults = LinkLayerParameters::default();
        Self {
            link_addr_size: defaults.link_addr_size,
            response_timeout: defaults.response_timeout,
            poll_interval: defaults.poll_interval,
        }
    }
}

impl LinkLayerParametersBuilder {
    /// Size of the link address field (1 or 2).
    pub fn link_addr_size(mut self, size: u8) -> Self {
        self.link_addr_size = size;
        self
    }

    /// Timeout waiting for a slave response (default 500ms).
    pub fn response_timeout(mut self, timeout: Duration) -> Self {
        self.response_timeout = timeout;
        self
    }

    /// Interval between polls when idle (default 1000ms).
    pub fn poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    /// Build and validate the parameters.
    ///
    /// # Constraints
    /// - `link_addr_size`: must be 1 or 2
    /// - `response_timeout`: must be non-zero
    /// - `poll_interval`: must be non-zero
    pub fn build(self) -> Result<LinkLayerParameters, ConfigError> {
        if self.link_addr_size != 1 && self.link_addr_size != 2 {
            return Err(ConfigError::InvalidLinkAddrSize(self.link_addr_size));
        }
        if self.response_timeout.is_zero() {
            return Err(ConfigError::ZeroTimeout("response_timeout"));
        }
        if self.poll_interval.is_zero() {
            return Err(ConfigError::ZeroTimeout("poll_interval"));
        }

        Ok(LinkLayerParameters {
            link_addr_size: self.link_addr_size,
            response_timeout: self.response_timeout,
            poll_interval: self.poll_interval,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults() {
        let params = LinkLayerParameters::builder().build().unwrap();
        assert_eq!(params.link_addr_size(), 1);
        assert_eq!(params.response_timeout(), Duration::from_millis(500));
        assert_eq!(params.poll_interval(), Duration::from_millis(1000));
    }

    #[test]
    fn custom_values() {
        let params = LinkLayerParameters::builder()
            .link_addr_size(2)
            .response_timeout(Duration::from_millis(1000))
            .poll_interval(Duration::from_millis(2000))
            .build()
            .unwrap();
        assert_eq!(params.link_addr_size(), 2);
        assert_eq!(params.response_timeout(), Duration::from_millis(1000));
        assert_eq!(params.poll_interval(), Duration::from_millis(2000));
    }

    #[test]
    fn rejects_invalid_link_addr_size() {
        assert!(LinkLayerParameters::builder()
            .link_addr_size(0)
            .build()
            .is_err());
        assert!(LinkLayerParameters::builder()
            .link_addr_size(3)
            .build()
            .is_err());
    }

    #[test]
    fn rejects_zero_response_timeout() {
        assert!(LinkLayerParameters::builder()
            .response_timeout(Duration::ZERO)
            .build()
            .is_err());
    }

    #[test]
    fn rejects_zero_poll_interval() {
        assert!(LinkLayerParameters::builder()
            .poll_interval(Duration::ZERO)
            .build()
            .is_err());
    }
}
