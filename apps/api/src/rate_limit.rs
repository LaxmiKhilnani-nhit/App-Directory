use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Sliding-window counter used by the auth endpoints to blunt password guessing.
///
/// In-memory by design: there is one API process, and losing the counters on restart is
/// an acceptable trade for having no extra infrastructure. Account lockout (which is
/// stored in the database) is the durable defence.
#[derive(Default)]
pub struct RateLimiter {
    windows: Mutex<HashMap<String, VecDeque<Instant>>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records an attempt and reports whether it is allowed. A `limit` of zero disables
    /// the check.
    pub fn allow(&self, key: &str, limit: usize, window: Duration) -> bool {
        if limit == 0 {
            return true;
        }

        let now = Instant::now();
        let mut windows = self.windows.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

        // Keys are IP-based, so the map is bounded by client count. Prune aggressively
        // anyway: stale keys cost nothing to drop and keep lookups fast.
        if windows.len() > 4096 {
            windows.retain(|_, queue| queue.back().is_some_and(|last| now.duration_since(*last) < window));
        }

        let queue = windows.entry(key.to_string()).or_default();
        while queue
            .front()
            .is_some_and(|first| now.duration_since(*first) > window)
        {
            queue.pop_front();
        }

        if queue.len() >= limit {
            return false;
        }

        queue.push_back(now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_up_to_the_limit() {
        let limiter = RateLimiter::new();
        let window = Duration::from_secs(60);

        for attempt in 1..=3 {
            assert!(limiter.allow("1.2.3.4", 3, window), "attempt {attempt} should pass");
        }
        assert!(!limiter.allow("1.2.3.4", 3, window));
    }

    #[test]
    fn keys_are_independent() {
        let limiter = RateLimiter::new();
        let window = Duration::from_secs(60);

        assert!(limiter.allow("10.0.0.1", 1, window));
        assert!(!limiter.allow("10.0.0.1", 1, window));
        assert!(limiter.allow("10.0.0.2", 1, window));
    }

    #[test]
    fn zero_limit_disables_the_check() {
        let limiter = RateLimiter::new();
        for _ in 0..50 {
            assert!(limiter.allow("anything", 0, Duration::from_secs(60)));
        }
    }

    #[test]
    fn old_attempts_expire() {
        let limiter = RateLimiter::new();
        let window = Duration::from_millis(20);

        assert!(limiter.allow("host", 1, window));
        assert!(!limiter.allow("host", 1, window));
        std::thread::sleep(Duration::from_millis(40));
        assert!(limiter.allow("host", 1, window));
    }
}
