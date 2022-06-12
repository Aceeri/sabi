use bevy::{ecs::entity::Entities, prelude::*, reflect::FromReflect};

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Networking resource to communicate what "game tick" things belong to.
#[derive(Debug, Clone, Serialize, Deserialize, Reflect, FromReflect)]
pub struct NetworkTick(u64);

impl NetworkTick {
    pub fn new() -> Self {
        Self(0)
    }

    pub fn tick(&mut self) {
        self.0 += 1;
    }

    pub fn set_tick(&mut self, tick: NetworkTick) {
        self.0 = tick.0;
    }

    pub fn current(&self) -> u64 {
        self.0
    }
}

/// Tick rate of the network sending/receiving
#[derive(Deref, DerefMut, Debug, Clone)]
pub struct NetworkGameTimer(pub Timer);

impl Default for NetworkGameTimer {
    fn default() -> Self {
        Self(Timer::new(tick_hz(32), true))
    }
}

impl NetworkGameTimer {
    pub fn new(tick_rate: Duration) -> Self {
        Self(Timer::new(tick_rate, true))
    }
}

/// Quick function for getting a duration for tick rates.
pub const fn tick_hz(rate: u64) -> Duration {
    Duration::from_nanos(1_000_000_000 / rate)
}

pub fn tick_network(
    mut tick: ResMut<NetworkTick>,
    time: Res<Time>,
    mut network_timer: ResMut<NetworkGameTimer>,
) {
    network_timer.tick(time.delta());

    if network_timer.just_finished() {
        tick.tick();
    }
}

pub fn on_network_tick(network_timer: Res<NetworkGameTimer>) -> bool {
    network_timer.just_finished()
}
