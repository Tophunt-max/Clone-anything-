//! Fixed-window rate limits kept in memory, per key (e.g. `login:ip:<hash>`). One API instance
//! is planned at first; a shared store replaces this when the API runs several instances.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limit {
    pub max: u32,
    pub per: Duration,
}

/// The limits the API applies. Tests turn them up or down.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Login and registration attempts per client address.
    pub auth_per_ip: Limit,
    /// Login attempts per email address, whoever sends them.
    pub login_per_email: Limit,
    /// Token refreshes per client address.
    pub refresh_per_ip: Limit,
}

impl Default for Limits {
    fn default() -> Self {
        let minute = Duration::from_secs(60);
        Limits {
            auth_per_ip: Limit { max: 20, per: minute },
            login_per_email: Limit { max: 10, per: Duration::from_secs(15 * 60) },
            refresh_per_ip: Limit { max: 60, per: minute },
        }
    }
}

const MAX_KEYS: usize = 100_000;

#[derive(Default)]
pub struct RateLimiter {
    windows: Mutex<HashMap<String, (Instant, u32)>>,
}

impl RateLimiter {
    /// Counts one hit for `key`. `Err(seconds)` when over the limit: how long until it resets.
    pub async fn hit(&self, key: &str, limit: Limit) -> Result<(), u64> {
        let now = Instant::now();
        let mut w = self.windows.lock().await;
        if w.len() >= MAX_KEYS {
            w.retain(|_, (start, _)| now.duration_since(*start) < limit.per);
        }
        let entry = w.entry(key.to_string()).or_insert((now, 0));
        if now.duration_since(entry.0) >= limit.per {
            *entry = (now, 0);
        }
        if entry.1 >= limit.max {
            let left = limit.per.saturating_sub(now.duration_since(entry.0));
            return Err(left.as_secs().max(1));
        }
        entry.1 = entry.1.saturating_add(1);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn allows_up_to_the_limit_then_refuses_with_a_wait() {
        let r = RateLimiter::default();
        let l = Limit { max: 2, per: Duration::from_secs(60) };
        assert!(r.hit("a", l).await.is_ok());
        assert!(r.hit("a", l).await.is_ok());
        let wait = r.hit("a", l).await.unwrap_err();
        assert!((1..=60).contains(&wait));
        assert!(r.hit("b", l).await.is_ok(), "keys are separate");
    }

    #[tokio::test]
    async fn the_window_resets() {
        let r = RateLimiter::default();
        let l = Limit { max: 1, per: Duration::from_millis(30) };
        assert!(r.hit("a", l).await.is_ok());
        assert!(r.hit("a", l).await.is_err());
        tokio::time::sleep(Duration::from_millis(40)).await;
        assert!(r.hit("a", l).await.is_ok());
    }
}
