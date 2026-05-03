//! Time provider abstraction.
//!
//! Most production code can call `Clock::now()` directly. Tests can swap in
//! a frozen or stepping clock to make time-sensitive assertions deterministic.

use chrono::{DateTime, Utc};
use std::sync::Arc;

pub trait Clock: Send + Sync + 'static {
    fn now(&self) -> DateTime<Utc>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

pub type SharedClock = Arc<dyn Clock>;

pub fn system_clock() -> SharedClock {
    Arc::new(SystemClock)
}

/// A `Clock` that always returns a fixed time. Used in tests
/// where timing needs to be deterministic — e.g. asserting that
/// idle-session-timeout fires after exactly N seconds without
/// having to actually sleep.
#[derive(Debug, Clone, Copy)]
pub struct MockClock(pub DateTime<Utc>);

impl MockClock {
    pub fn at(t: DateTime<Utc>) -> Self {
        Self(t)
    }
}

impl Clock for MockClock {
    fn now(&self) -> DateTime<Utc> {
        self.0
    }
}
