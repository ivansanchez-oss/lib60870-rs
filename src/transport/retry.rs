use std::time::Duration;

use tokio::time::sleep;
use tracing::{info, warn};

use super::{Connector, PhysLayer};

/// Strategy for reconnection with exponential backoff.
#[derive(Debug, Clone)]
pub struct RetryStrategy {
    pub min_delay: Duration,
    pub max_delay: Duration,
}

impl Default for RetryStrategy {
    fn default() -> Self {
        Self {
            min_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(30),
        }
    }
}

impl RetryStrategy {
    /// Returns an iterator that yields exponentially increasing delays,
    /// starting at `min_delay` and capped at `max_delay`.
    pub fn backoff_iter(&self) -> impl Iterator<Item = Duration> {
        let min = self.min_delay;
        let max = self.max_delay;
        (0u32..).map(move |i| {
            let delay = min.saturating_mul(1 << i.min(31));
            delay.min(max)
        })
    }
}

/// Retries connecting indefinitely using exponential backoff.
///
/// Never returns an error — it keeps retrying until a connection succeeds.
/// Each failure and the subsequent delay are logged via `tracing`.
pub async fn connect_with_retry<C: Connector>(connector: &C, retry: &RetryStrategy) -> PhysLayer {
    let mut delays = retry.backoff_iter();
    loop {
        match connector.connect().await {
            Ok(phys) => {
                info!("connection established");
                return phys;
            }
            Err(err) => {
                let delay = delays.next().expect("infinite iterator");
                warn!(?err, ?delay, "connection failed, retrying after delay");
                sleep(delay).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_iter_starts_at_min() {
        let strategy = RetryStrategy {
            min_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(30),
        };
        let delays: Vec<_> = strategy.backoff_iter().take(6).collect();
        assert_eq!(delays[0], Duration::from_secs(1));
        assert_eq!(delays[1], Duration::from_secs(2));
        assert_eq!(delays[2], Duration::from_secs(4));
        assert_eq!(delays[3], Duration::from_secs(8));
        assert_eq!(delays[4], Duration::from_secs(16));
        assert_eq!(delays[5], Duration::from_secs(30)); // capped
    }

    #[test]
    fn backoff_iter_caps_at_max() {
        let strategy = RetryStrategy {
            min_delay: Duration::from_millis(100),
            max_delay: Duration::from_millis(500),
        };
        let delays: Vec<_> = strategy.backoff_iter().take(10).collect();
        for delay in &delays[3..] {
            assert!(*delay <= Duration::from_millis(500));
        }
        // Last values should all be capped
        assert_eq!(*delays.last().unwrap(), Duration::from_millis(500));
    }

    #[tokio::test]
    async fn connect_with_retry_succeeds_after_failures() {
        use std::io;
        use std::sync::atomic::{AtomicU32, Ordering};

        struct MockConnector {
            attempts: AtomicU32,
            fail_count: u32,
        }

        impl Connector for MockConnector {
            async fn connect(&self) -> io::Result<PhysLayer> {
                let attempt = self.attempts.fetch_add(1, Ordering::SeqCst);
                if attempt < self.fail_count {
                    Err(io::Error::new(io::ErrorKind::ConnectionRefused, "mock failure"))
                } else {
                    // Return a PhysLayer wrapping a duplex stream
                    let (a, _b) = tokio::io::duplex(64);
                    Ok(PhysLayer::new(a))
                }
            }
        }

        let connector = MockConnector {
            attempts: AtomicU32::new(0),
            fail_count: 3,
        };
        let retry = RetryStrategy {
            min_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(10),
        };

        let _phys = connect_with_retry(&connector, &retry).await;
        assert_eq!(connector.attempts.load(Ordering::SeqCst), 4); // 3 failures + 1 success
    }
}
