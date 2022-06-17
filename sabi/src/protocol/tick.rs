use bevy::{prelude::*, reflect::FromReflect};

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

    pub fn set_tick(&mut self, tick: u64) {
        self.0 = tick;
    }

    pub fn current(&self) -> u64 {
        self.0
    }
}

/// Bitset of previous ticks that were successfully retrieved from the server.
#[derive(Debug, Clone, Serialize, Deserialize, Reflect, FromReflect)]
pub struct NetworkAck {
    base: NetworkTick,
    ack: u32,
}

impl NetworkAck {
    pub fn new(base: NetworkTick) -> Self {
        Self { base: base, ack: 0 }
    }

    pub fn ack(&mut self, tick: &NetworkTick) {
        let diff = self.base.current() as i64 - tick.current() as i64 - 1;
        println!("base: {:?}, tick: {:?}, diff: {:?}", self.base, tick, diff);
        if diff > 0 && diff <= 32 {
            self.ack |= 1 << diff;
        }
    }

    pub fn apply_ack(&mut self, ack: &NetworkAck) {
        let base_diff = self.base.current() as i64 - ack.base.current() as i64;
        if base_diff > 0 {
            self.ack |= ack.ack << base_diff;
        }
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn ack() {
        let ticks = (0..=20u64)
            .map(|num| {
                let mut tick = NetworkTick::new();
                tick.set_tick(num);
                tick
            })
            .collect::<Vec<_>>();

        let mut current_tick = NetworkTick::new();
        current_tick.set_tick(21);

        let mut ack = NetworkAck::new(current_tick);
        for tick in ticks {
            ack.ack(&tick);
        }
        println!("{:b}", ack.ack);
    }

    #[test]
    pub fn apply_ack() {
        let ticks = (0..=20u64)
            .map(|num| {
                let mut tick = NetworkTick::new();
                tick.set_tick(num);
                tick
            })
            .collect::<Vec<_>>();

        let mut current_tick = NetworkTick::new();
        current_tick.set_tick(21);
        let mut ack = NetworkAck::new(current_tick);

        let mut other_tick = NetworkTick::new();
        other_tick.set_tick(11);
        let mut other_ack = NetworkAck::new(other_tick);
        for tick in ticks {
            other_ack.ack(&tick);
        }

        ack.apply_ack(&other_ack);
        println!("{:b}", ack.ack);
    }
}
