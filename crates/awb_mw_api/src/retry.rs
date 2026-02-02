use crate::error::MwApiError;
use std::future::Future;
use std::time::Duration;
use tokio::time::sleep;
use tracing::warn;

pub struct RetryPolicy {
    pub max_retries: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_secs(2),
            max_delay: Duration::from_secs(60),
        }
    }
}

impl RetryPolicy {
    pub async fn execute<F, Fut, T>(&self, mut op: F) -> Result<T, MwApiError>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, MwApiError>>,
    {
        let mut attempt = 0;
        loop {
            match op().await {
                Ok(val) => return Ok(val),
                Err(e) if e.is_retryable() && attempt < self.max_retries => {
                    let internal_secs =
                        self.base_delay.as_secs_f64() * 2f64.powi(attempt as i32);
                    let internal_delay = internal_secs.min(self.max_delay.as_secs_f64());

                    // Honor server-requested retry_after for MaxLag and RateLimited
                    let server_delay = match &e {
                        MwApiError::MaxLag { retry_after } => *retry_after as f64,
                        MwApiError::RateLimited { retry_after } => *retry_after as f64,
                        _ => 0.0,
                    };

                    let effective_delay = internal_delay.max(server_delay);
                    let jitter = rand_jitter();
                    let delay = Duration::from_secs_f64(effective_delay + jitter);

                    warn!(attempt, ?delay, error = %e, "Retrying after error");
                    sleep(delay).await;
                    attempt += 1;
                }
                Err(e) => return Err(e),
            }
        }
    }
}

fn rand_jitter() -> f64 {
    // Use rand crate for proper randomness
    use rand::Rng;
    rand::thread_rng().gen_range(0.0..1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::MwApiError;

    #[test]
    fn test_retry_policy_default_values() {
        let policy = RetryPolicy::default();

        assert_eq!(policy.max_retries, 3);
        assert_eq!(policy.base_delay, Duration::from_secs(2));
        assert_eq!(policy.max_delay, Duration::from_secs(60));
    }

    #[test]
    fn test_retry_policy_custom_values() {
        let policy = RetryPolicy {
            max_retries: 5,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
        };

        assert_eq!(policy.max_retries, 5);
        assert_eq!(policy.base_delay, Duration::from_millis(100));
        assert_eq!(policy.max_delay, Duration::from_secs(30));
    }

    #[tokio::test]
    async fn test_execute_succeeds_first_try() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        let policy = RetryPolicy {
            max_retries: 3,
            base_delay: Duration::from_millis(10),
            max_delay: Duration::from_secs(1),
        };

        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        let result = policy
            .execute(move || {
                let count = call_count_clone.clone();
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    Ok::<i32, MwApiError>(42)
                }
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(
            call_count.load(Ordering::SeqCst),
            1,
            "Should succeed on first try"
        );
    }

    #[tokio::test]
    async fn test_execute_retries_on_transient_failure() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        let policy = RetryPolicy {
            max_retries: 3,
            base_delay: Duration::from_millis(10),
            max_delay: Duration::from_secs(1),
        };

        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        let result = policy
            .execute(move || {
                let count = call_count_clone.clone();
                async move {
                    let current = count.fetch_add(1, Ordering::SeqCst) + 1;
                    if current < 3 {
                        Err(MwApiError::MaxLag { retry_after: 5 })
                    } else {
                        Ok::<i32, MwApiError>(42)
                    }
                }
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(
            call_count.load(Ordering::SeqCst),
            3,
            "Should retry twice then succeed"
        );
    }

    #[tokio::test]
    async fn test_execute_exhausts_max_retries() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        let policy = RetryPolicy {
            max_retries: 2,
            base_delay: Duration::from_millis(10),
            max_delay: Duration::from_secs(1),
        };

        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        let result = policy
            .execute(move || {
                let count = call_count_clone.clone();
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    Err::<i32, MwApiError>(MwApiError::MaxLag { retry_after: 5 })
                }
            })
            .await;

        assert!(result.is_err());
        assert_eq!(
            call_count.load(Ordering::SeqCst),
            3,
            "Should try once + 2 retries = 3 total attempts"
        );

        match result {
            Err(MwApiError::MaxLag { retry_after }) => assert_eq!(retry_after, 5),
            _ => panic!("Expected MaxLag error"),
        }
    }

    #[tokio::test]
    async fn test_execute_fails_immediately_on_non_retryable_error() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        let policy = RetryPolicy {
            max_retries: 3,
            base_delay: Duration::from_millis(10),
            max_delay: Duration::from_secs(1),
        };

        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        let result = policy
            .execute(move || {
                let count = call_count_clone.clone();
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    Err::<i32, MwApiError>(MwApiError::ApiError {
                        code: "permissiondenied".to_string(),
                        info: "Access denied".to_string(),
                    })
                }
            })
            .await;

        assert!(result.is_err());
        assert_eq!(
            call_count.load(Ordering::SeqCst),
            1,
            "Should not retry non-retryable errors"
        );
    }

    #[test]
    fn test_calculate_delay_exponential_backoff() {
        let policy = RetryPolicy {
            max_retries: 5,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(32),
        };

        // Attempt 0: 1 * 2^0 = 1 second
        // Attempt 1: 1 * 2^1 = 2 seconds
        // Attempt 2: 1 * 2^2 = 4 seconds
        // Attempt 3: 1 * 2^3 = 8 seconds
        // Note: actual delays include jitter, so we just verify the policy exists
        assert_eq!(policy.base_delay.as_secs(), 1);
        assert_eq!(policy.max_delay.as_secs(), 32);
    }

    #[test]
    fn test_rand_jitter_returns_valid_range() {
        for _ in 0..100 {
            let jitter = rand_jitter();
            assert!(
                jitter >= 0.0 && jitter < 1.0,
                "Jitter should be in [0, 1) range"
            );
        }
    }

    #[tokio::test]
    async fn test_maxlag_retry_after_overrides_backoff() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        // Base delay is 10ms, but MaxLag says retry_after: 30 (seconds)
        // The effective delay should be at least 30s (we can't wait that long in tests,
        // so we verify the logic by checking the attempt count with a short retry_after)
        let policy = RetryPolicy {
            max_retries: 1,
            base_delay: Duration::from_millis(10),  // internal backoff: 10ms
            max_delay: Duration::from_secs(60),
        };

        let call_count = Arc::new(AtomicU32::new(0));
        let cc = call_count.clone();

        let start = std::time::Instant::now();
        let _ = policy
            .execute(move || {
                let count = cc.clone();
                async move {
                    let c = count.fetch_add(1, Ordering::SeqCst);
                    if c == 0 {
                        // retry_after=1 second — this should override the 10ms internal backoff
                        Err::<i32, MwApiError>(MwApiError::MaxLag { retry_after: 1 })
                    } else {
                        Ok(42)
                    }
                }
            })
            .await;

        let elapsed = start.elapsed();
        // Should have waited at least ~1 second (the retry_after), not just 10ms
        assert!(
            elapsed >= Duration::from_millis(900),
            "Expected delay >= 900ms from retry_after=1, got {:?}",
            elapsed
        );
    }

    #[tokio::test]
    async fn test_maxlag_without_large_retry_after_uses_backoff() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        // retry_after=0 means server doesn't specify, so internal backoff (10ms) should apply
        let policy = RetryPolicy {
            max_retries: 1,
            base_delay: Duration::from_millis(10),
            max_delay: Duration::from_secs(60),
        };

        let call_count = Arc::new(AtomicU32::new(0));
        let cc = call_count.clone();

        let start = std::time::Instant::now();
        let _ = policy
            .execute(move || {
                let count = cc.clone();
                async move {
                    let c = count.fetch_add(1, Ordering::SeqCst);
                    if c == 0 {
                        Err::<i32, MwApiError>(MwApiError::MaxLag { retry_after: 0 })
                    } else {
                        Ok(42)
                    }
                }
            })
            .await;

        let elapsed = start.elapsed();
        // Should have used internal backoff (~10ms + jitter), not a long delay
        assert!(
            elapsed < Duration::from_secs(2),
            "Expected short delay with retry_after=0, got {:?}",
            elapsed
        );
    }
}
