use crate::error::MwApiError;
use std::time::Duration;
use std::future::Future;
use tokio::time::sleep;
use tracing::warn;

pub struct RetryPolicy {
    pub max_retries: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self { max_retries: 3, base_delay: Duration::from_secs(2), max_delay: Duration::from_secs(60) }
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
                    let delay_secs = self.base_delay.as_secs_f64() * 2f64.powi(attempt as i32);
                    let jitter = rand_jitter();
                    let delay = Duration::from_secs_f64(delay_secs.min(self.max_delay.as_secs_f64()) + jitter);
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
    // Simple deterministic jitter based on current time nanoseconds
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    (ns % 1000) as f64 / 1000.0
}
