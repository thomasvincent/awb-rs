use awb_domain::profile::ThrottlePolicy;
use tokio::sync::Mutex;
use tokio::time::{Instant, sleep};

pub struct ThrottleController {
    policy: ThrottlePolicy,
    last_edit: Mutex<Option<Instant>>,
}

impl ThrottleController {
    pub fn new(policy: ThrottlePolicy) -> Self {
        Self {
            policy,
            last_edit: Mutex::new(None),
        }
    }

    pub async fn acquire_edit_permit(&self) {
        let mut last = self.last_edit.lock().await;
        if let Some(prev) = *last {
            let elapsed = prev.elapsed();
            if elapsed < self.policy.min_edit_interval {
                sleep(self.policy.min_edit_interval - elapsed).await;
            }
        }
        *last = Some(Instant::now());
    }

    pub fn maxlag(&self) -> u32 {
        self.policy.maxlag
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_throttle_controller_new_with_various_intervals() {
        let policy1 = ThrottlePolicy {
            min_edit_interval: Duration::from_secs(5),
            maxlag: 5,
            max_retries: 3,
            backoff_base: Duration::from_secs(2),
        };

        let controller1 = ThrottleController::new(policy1.clone());
        assert_eq!(controller1.maxlag(), 5);

        let policy2 = ThrottlePolicy {
            min_edit_interval: Duration::from_millis(100),
            maxlag: 10,
            max_retries: 5,
            backoff_base: Duration::from_millis(500),
        };

        let controller2 = ThrottleController::new(policy2);
        assert_eq!(controller2.maxlag(), 10);
    }

    #[tokio::test]
    async fn test_acquire_edit_permit_first_call() {
        let policy = ThrottlePolicy {
            min_edit_interval: Duration::from_millis(100),
            maxlag: 5,
            max_retries: 3,
            backoff_base: Duration::from_secs(2),
        };

        let controller = ThrottleController::new(policy);

        let start = tokio::time::Instant::now();
        controller.acquire_edit_permit().await;
        let elapsed = start.elapsed();

        // First call should not wait
        assert!(
            elapsed < Duration::from_millis(50),
            "First call should be immediate"
        );
    }

    #[tokio::test]
    async fn test_acquire_edit_permit_respects_min_edit_interval() {
        let min_interval = Duration::from_millis(100);
        let policy = ThrottlePolicy {
            min_edit_interval: min_interval,
            maxlag: 5,
            max_retries: 3,
            backoff_base: Duration::from_secs(2),
        };

        let controller = ThrottleController::new(policy);

        // First permit - should be immediate
        controller.acquire_edit_permit().await;

        // Second permit - should wait for min_edit_interval
        let start = tokio::time::Instant::now();
        controller.acquire_edit_permit().await;
        let elapsed = start.elapsed();

        // Should wait approximately min_interval (allow for timing variations)
        assert!(
            elapsed >= Duration::from_millis(90),
            "Should wait at least 90ms"
        );
        assert!(
            elapsed < Duration::from_millis(200),
            "Should not wait more than 200ms"
        );
    }

    #[tokio::test]
    async fn test_acquire_edit_permit_multiple_calls() {
        let min_interval = Duration::from_millis(50);
        let policy = ThrottlePolicy {
            min_edit_interval: min_interval,
            maxlag: 5,
            max_retries: 3,
            backoff_base: Duration::from_secs(2),
        };

        let controller = ThrottleController::new(policy);

        let start = tokio::time::Instant::now();

        // Make 3 calls
        controller.acquire_edit_permit().await;
        controller.acquire_edit_permit().await;
        controller.acquire_edit_permit().await;

        let elapsed = start.elapsed();

        // Total time should be at least 2 * min_interval (3 calls = 2 waits)
        assert!(
            elapsed >= Duration::from_millis(100),
            "Should wait at least 100ms for 3 calls"
        );
    }

    #[tokio::test]
    async fn test_acquire_edit_permit_no_wait_if_interval_passed() {
        let min_interval = Duration::from_millis(50);
        let policy = ThrottlePolicy {
            min_edit_interval: min_interval,
            maxlag: 5,
            max_retries: 3,
            backoff_base: Duration::from_secs(2),
        };

        let controller = ThrottleController::new(policy);

        // First permit
        controller.acquire_edit_permit().await;

        // Wait longer than min_interval
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Second permit should be immediate since we already waited
        let start = tokio::time::Instant::now();
        controller.acquire_edit_permit().await;
        let elapsed = start.elapsed();

        assert!(
            elapsed < Duration::from_millis(20),
            "Should not wait if interval already passed"
        );
    }

    #[test]
    fn test_maxlag_returns_correct_value() {
        let policy = ThrottlePolicy {
            min_edit_interval: Duration::from_secs(5),
            maxlag: 15,
            max_retries: 3,
            backoff_base: Duration::from_secs(2),
        };

        let controller = ThrottleController::new(policy);
        assert_eq!(controller.maxlag(), 15);
    }
}
