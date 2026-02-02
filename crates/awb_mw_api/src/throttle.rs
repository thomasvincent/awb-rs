use awb_domain::profile::ThrottlePolicy;
use tokio::sync::Mutex;
use tokio::time::{Instant, sleep};

pub struct ThrottleController {
    policy: ThrottlePolicy,
    last_edit: Mutex<Option<Instant>>,
}

impl ThrottleController {
    pub fn new(policy: ThrottlePolicy) -> Self {
        Self { policy, last_edit: Mutex::new(None) }
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

    pub fn maxlag(&self) -> u32 { self.policy.maxlag }
}
