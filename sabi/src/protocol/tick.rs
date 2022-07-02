use bevy::{prelude::*, reflect::FromReflect};
use smallvec::SmallVec;

use std::{time::Duration};

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
        let diff = self.base.tick() as i64 - tick.tick() as i64 - 1;
        if diff >= 0 && diff <= 32 {
            self.ack |= 1 << diff;
        }
    }

    pub fn apply_ack(&mut self, ack: &NetworkAck) {
        let base_diff = self.base.tick() as i64 - ack.base.tick() as i64;
        if base_diff > 0 {
            self.ack |= ack.ack << base_diff;
        }
    }

    /// Sets the new base of this ack and returns any unacked ticks
    pub fn set_base(&mut self, new_base: NetworkTick) -> SmallVec<[NetworkTick; 4]> {
        let mut unacked = SmallVec::new();

        let base_diff = new_base.tick() as i64 - self.base.tick() as i64;
        for index in ((32 - base_diff).max(0)..32).rev() {
            let tick_num = self.base.tick() as i64 - index as i64;
            if tick_num > 0 {
                let tick = NetworkTick::new(tick_num as u64 - 1);
                if self.ack & (1 << index) == 0 {
                    unacked.push(tick);
                }
            }
        }

        if base_diff >= 32 {
            self.ack = 0;
            unacked
                .extend((self.base.tick() + 32..new_base.tick()).map(|num| NetworkTick::new(num)));
        } else if base_diff > 0 {
            self.ack = self.ack << base_diff;
        }

        self.base = new_base;
        //unacked.sort();
        unacked
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

pub fn tick_network_timer(
    time: Res<Time>,
    mut network_timer: ResMut<NetworkGameTimer>,
) {
    network_timer.tick(time.delta());
}

pub fn increment_network_tick(
    mut tick: ResMut<NetworkTick>,
) {
    tick.increment_tick();
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
            .map(|num| NetworkTick::new(num))
            .collect::<Vec<_>>();

        let current_tick = NetworkTick::new(21);

        let mut ack = NetworkAck::new(current_tick);
        for tick in ticks {
            ack.ack(&tick);
        }
        println!("{:b}", ack.ack);
    }

    #[test]
    pub fn apply_ack() {
        let ticks = (0..=20u64)
            .map(|num| NetworkTick::new(num))
            .collect::<Vec<_>>();

        let current_tick = NetworkTick::new(21);
        let mut ack = NetworkAck::new(current_tick);

        let other_tick = NetworkTick::new(11);
        let mut other_ack = NetworkAck::new(other_tick);
        for tick in ticks {
            other_ack.ack(&tick);
        }

        ack.apply_ack(&other_ack);
        println!("{:b}", ack.ack);
    }

    #[test]
    pub fn set_base() {
        let ticks = (0..=20u64)
            .map(|num| NetworkTick::new(num))
            .collect::<Vec<_>>();

        let current_tick = NetworkTick::new(21);

        let mut ack = NetworkAck::new(current_tick);
        for tick in &ticks[2..] {
            ack.ack(&tick);
        }

        let unacked = ack.set_base(NetworkTick::new(35));
        assert_eq!(unacked.as_slice(), &ticks[..2]);
        let unacked = ack.set_base(NetworkTick::new(65));

        let extended_unacked = (21..=32)
            .map(|num| NetworkTick::new(num))
            .collect::<Vec<_>>();
        assert_eq!(unacked.as_slice(), extended_unacked.as_slice());
    }
}
