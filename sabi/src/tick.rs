use bevy::{prelude::*, reflect::FromReflect};

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Networking resource to communicate what "game tick" things belong to.
#[derive(
    Debug,
    Copy,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    Reflect,
    FromReflect,
)]
pub struct NetworkTick(u64);

impl Default for NetworkTick {
    fn default() -> Self {
        Self::new(0)
    }
}

impl NetworkTick {
    pub fn new(tick: u64) -> Self {
        Self(tick)
    }

    pub fn increment_tick(&mut self) {
        self.0 += 1;
    }

    pub fn set_tick(&mut self, tick: u64) {
        self.0 = tick;
    }

    pub fn tick(&self) -> u64 {
        self.0
    }
}

/// Quick function for getting a duration for tick rates.
pub const fn tick_hz(rate: u64) -> Duration {
    Duration::from_nanos(1_000_000_000 / rate)
}
