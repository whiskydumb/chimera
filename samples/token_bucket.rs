//! a minimal token-bucket rate limiter.

use std::time::Instant;

pub struct TokenBucket {
    capacity: f64,
    tokens: f64,
    refill_per_sec: f64,
    last: Instant,
}

impl TokenBucket {
    pub fn new(capacity: f64, refill_per_sec: f64) -> Self {
        Self { capacity, tokens: capacity, refill_per_sec, last: Instant::now() }
    }

    /// tries to take `amount` tokens, refilling based on elapsed time first.
    pub fn try_acquire(&mut self, amount: f64) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_per_sec).min(self.capacity);
        self.last = now;
        if self.tokens >= amount {
            self.tokens -= amount;
            true
        } else {
            false
        }
    }
}
