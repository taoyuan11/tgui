//! Time contracts shared by animation and deterministic tests.
//!
//! Animation scheduling is UI-thread-owned. A clock may be read by consumers,
//! but it never mutates the UI tree directly.

use std::time::{Duration, Instant};

pub trait FrameClock {
    fn now(&self) -> Duration;
}

#[derive(Debug)]
pub struct SystemClock {
    origin: Instant,
}

impl SystemClock {
    pub fn new() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameClock for SystemClock {
    fn now(&self) -> Duration {
        self.origin.elapsed()
    }
}
