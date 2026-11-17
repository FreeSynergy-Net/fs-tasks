//! Time: the layer's clock, made strictly increasing for writes.
//!
//! Pattern: Decorator over the layer's `Clock` (a test moves the clock; two writes never share one millisecond, so
//! "last written first" and the deadlines are exact even when the clock stands still).

use std::sync::atomic::{AtomicI64, Ordering};

pub use fs_objects::v5::Clock;

/// The clock of one provider.
pub struct Ticker {
    clock: Clock,
    last: AtomicI64,
}

impl Ticker {
    /// Wrap a clock.
    #[must_use]
    pub fn new(clock: Clock) -> Self {
        Self {
            clock,
            last: AtomicI64::new(i64::MIN),
        }
    }

    /// The time of the next write: the clock, but never at or before an earlier write.
    pub fn tick(&self) -> i64 {
        let now = self.clock.now();
        let mut prev = self.last.load(Ordering::SeqCst);
        loop {
            let next = now.max(prev.saturating_add(1));
            match self
                .last
                .compare_exchange(prev, next, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => return next,
                Err(seen) => prev = seen,
            }
        }
    }

    /// The time for a read (deadlines): the clock, or the last write when that is later.
    #[must_use]
    pub fn peek(&self) -> i64 {
        self.clock.now().max(self.last.load(Ordering::SeqCst))
    }
}
