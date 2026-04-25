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
