use std::time::Duration;

/// APCI (Application Protocol Control Information) parameters for CS 104.
///
/// Controls connection timeouts and flow control windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApciParameters {
    /// Max unconfirmed I-frames sent before waiting for ack.
    pub k: u16,
    /// I-frames received before sending S-frame ack.
    pub w: u16,
    /// Timeout for connection establishment (T0).
    pub t0: Duration,
    /// Timeout for send or test APDUs (T1).
    pub t1: Duration,
    /// Timeout for acknowledges in case of no data (T2). Must be < T1.
    pub t2: Duration,
    /// Timeout for sending test frames when idle (T3).
    pub t3: Duration,
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

/// Default IEC 60870-5-104 port.
pub const DEFAULT_PORT: u16 = 2404;

/// Default TLS port for IEC 62351.
pub const DEFAULT_TLS_PORT: u16 = 19998;
