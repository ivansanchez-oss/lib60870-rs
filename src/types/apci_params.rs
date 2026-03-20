use std::time::Duration;

use crate::error::{Error, Result};

/// APCI (Application Protocol Control Information) parameters for CS 104.
///
/// Controls connection timeouts and flow control windows.
/// Use [`ApciParametersBuilder`] to construct with validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApciParameters {
    k: u16,
    w: u16,
    t0: Duration,
    t1: Duration,
    t2: Duration,
    t3: Duration,
}

impl ApciParameters {
    /// Max unconfirmed I-frames sent before waiting for ack.
    pub fn k(&self) -> u16 {
        self.k
    }

    /// I-frames received before sending S-frame ack.
    pub fn w(&self) -> u16 {
        self.w
    }

    /// Timeout for connection establishment (T0).
    pub fn t0(&self) -> Duration {
        self.t0
    }

    /// Timeout for send or test APDUs (T1).
    pub fn t1(&self) -> Duration {
        self.t1
    }

    /// Timeout for acknowledges in case of no data (T2). Must be < T1.
    pub fn t2(&self) -> Duration {
        self.t2
    }

    /// Timeout for sending test frames when idle (T3).
    pub fn t3(&self) -> Duration {
        self.t3
    }

    /// Create a builder initialized with default values.
    pub fn builder() -> ApciParametersBuilder {
        ApciParametersBuilder::default()
    }
}

impl Default for ApciParameters {
    fn default() -> Self {
        Self {
            k: 12,
            w: 8,
            t0: Duration::from_secs(10),
            t1: Duration::from_secs(15),
            t2: Duration::from_secs(10),
            t3: Duration::from_secs(20),
        }
    }
}

/// Builder for [`ApciParameters`] with IEC 60870-5-104 constraint validation.
///
/// All fields start with standard default values.
#[derive(Debug, Clone)]
pub struct ApciParametersBuilder {
    k: u16,
    w: u16,
    t0: Duration,
    t1: Duration,
    t2: Duration,
    t3: Duration,
}

impl Default for ApciParametersBuilder {
    fn default() -> Self {
        let defaults = ApciParameters::default();
        Self {
            k: defaults.k,
            w: defaults.w,
            t0: defaults.t0,
            t1: defaults.t1,
            t2: defaults.t2,
            t3: defaults.t3,
        }
    }
}

impl ApciParametersBuilder {
    pub fn k(mut self, k: u16) -> Self {
        self.k = k;
        self
    }

    pub fn w(mut self, w: u16) -> Self {
        self.w = w;
        self
    }

    pub fn t0(mut self, t0: Duration) -> Self {
        self.t0 = t0;
        self
    }

    pub fn t1(mut self, t1: Duration) -> Self {
        self.t1 = t1;
        self
    }

    pub fn t2(mut self, t2: Duration) -> Self {
        self.t2 = t2;
        self
    }

    pub fn t3(mut self, t3: Duration) -> Self {
        self.t3 = t3;
        self
    }

    /// Build and validate the parameters.
    ///
    /// # Constraints (IEC 60870-5-104)
    /// - `k` must be in 1..=32767
    /// - `w` must be in 1..=32767
    /// - `w` must not exceed `k`
    /// - `t2` must be less than `t1`
    /// - All timeouts must be non-zero
    pub fn build(self) -> Result<ApciParameters> {
        if self.k == 0 || self.k > 32767 {
            return Err(Error::InvalidParameter(format!(
                "k must be in 1..=32767, got {}",
                self.k
            )));
        }
        if self.w == 0 || self.w > 32767 {
            return Err(Error::InvalidParameter(format!(
                "w must be in 1..=32767, got {}",
                self.w
            )));
        }
        if self.w > self.k {
            return Err(Error::InvalidParameter(format!(
                "w ({}) must not exceed k ({})",
                self.w, self.k
            )));
        }
        if self.t0.is_zero() {
            return Err(Error::InvalidParameter("t0 must be non-zero".into()));
        }
        if self.t1.is_zero() {
            return Err(Error::InvalidParameter("t1 must be non-zero".into()));
        }
        if self.t2.is_zero() {
            return Err(Error::InvalidParameter("t2 must be non-zero".into()));
        }
        if self.t3.is_zero() {
            return Err(Error::InvalidParameter("t3 must be non-zero".into()));
        }
        if self.t2 >= self.t1 {
            return Err(Error::InvalidParameter(format!(
                "t2 ({:?}) must be less than t1 ({:?})",
                self.t2, self.t1
            )));
        }

        Ok(ApciParameters {
            k: self.k,
            w: self.w,
            t0: self.t0,
            t1: self.t1,
            t2: self.t2,
            t3: self.t3,
        })
    }
}

/// Default IEC 60870-5-104 port.
pub const DEFAULT_PORT: u16 = 2404;

/// Default TLS port for IEC 62351.
pub const DEFAULT_TLS_PORT: u16 = 19998;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_valid() {
        let params = ApciParameters::default();
        assert_eq!(params.k(), 12);
        assert_eq!(params.w(), 8);
        assert!(params.t2() < params.t1());
    }

    #[test]
    fn builder_defaults() {
        let params = ApciParameters::builder().build().unwrap();
        assert_eq!(params, ApciParameters::default());
    }

    #[test]
    fn builder_custom() {
        let params = ApciParameters::builder()
            .k(20)
            .w(10)
            .t1(Duration::from_secs(20))
            .t2(Duration::from_secs(12))
            .build()
            .unwrap();
        assert_eq!(params.k(), 20);
        assert_eq!(params.w(), 10);
    }

    #[test]
    fn rejects_k_zero() {
        assert!(ApciParameters::builder().k(0).build().is_err());
    }

    #[test]
    fn rejects_k_too_large() {
        assert!(ApciParameters::builder().k(32768).build().is_err());
    }

    #[test]
    fn rejects_w_zero() {
        assert!(ApciParameters::builder().w(0).build().is_err());
    }

    #[test]
    fn rejects_w_greater_than_k() {
        assert!(ApciParameters::builder().k(10).w(11).build().is_err());
    }

    #[test]
    fn rejects_t2_ge_t1() {
        assert!(ApciParameters::builder()
            .t1(Duration::from_secs(10))
            .t2(Duration::from_secs(10))
            .build()
            .is_err());

        assert!(ApciParameters::builder()
            .t1(Duration::from_secs(10))
            .t2(Duration::from_secs(15))
            .build()
            .is_err());
    }

    #[test]
    fn rejects_zero_timeouts() {
        assert!(ApciParameters::builder()
            .t0(Duration::ZERO)
            .build()
            .is_err());
        assert!(ApciParameters::builder()
            .t1(Duration::ZERO)
            .build()
            .is_err());
        assert!(ApciParameters::builder()
            .t3(Duration::ZERO)
            .build()
            .is_err());
    }
}
