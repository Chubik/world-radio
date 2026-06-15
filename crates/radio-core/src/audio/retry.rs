use std::time::Duration;

const MAX_ATTEMPTS: u32 = 5;
const BASE_MS: u64 = 500;
const CAP_MS: u64 = 8000;

pub struct Backoff {
    attempt: u32,
}

impl Backoff {
    pub fn new() -> Self {
        Self { attempt: 0 }
    }

    pub fn next_delay(&mut self) -> Option<Duration> {
        if self.attempt >= MAX_ATTEMPTS {
            return None;
        }
        let shift = self.attempt;
        self.attempt += 1;
        let ms = BASE_MS.saturating_mul(1u64 << shift).min(CAP_MS);
        Some(Duration::from_millis(ms))
    }

    pub fn reset(&mut self) {
        self.attempt = 0;
    }

    pub fn attempt(&self) -> u32 {
        self.attempt
    }
}

impl Default for Backoff {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delays_grow_exponentially_then_cap() {
        let mut b = Backoff::new();
        assert_eq!(b.next_delay(), Some(Duration::from_millis(500)));
        assert_eq!(b.next_delay(), Some(Duration::from_millis(1000)));
        assert_eq!(b.next_delay(), Some(Duration::from_millis(2000)));
        assert_eq!(b.next_delay(), Some(Duration::from_millis(4000)));
        assert_eq!(b.next_delay(), Some(Duration::from_millis(8000)));
    }

    #[test]
    fn stops_after_max_attempts() {
        let mut b = Backoff::new();
        for _ in 0..MAX_ATTEMPTS {
            assert!(b.next_delay().is_some());
        }
        assert_eq!(b.next_delay(), None);
    }

    #[test]
    fn reset_restarts_schedule() {
        let mut b = Backoff::new();
        b.next_delay();
        b.next_delay();
        b.reset();
        assert_eq!(b.next_delay(), Some(Duration::from_millis(500)));
    }

    #[test]
    fn attempt_reports_count() {
        let mut b = Backoff::new();
        assert_eq!(b.attempt(), 0);
        b.next_delay();
        assert_eq!(b.attempt(), 1);
        b.reset();
        assert_eq!(b.attempt(), 0);
    }
}
