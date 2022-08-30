use bevy::{prelude::*, reflect::FromReflect};
use std::collections::{btree_map::Entry, BTreeMap};

use serde::{Deserialize, Serialize};

use super::{ClientId, NetworkTick};

#[derive(Default, Clone)]
pub struct ClientAcks {
    acks: BTreeMap<ClientId, NetworkAck>,
}

impl ClientAcks {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply_ack(&mut self, client_id: ClientId, ack: &NetworkAck) {
        match self.acks.entry(client_id) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().apply_ack(ack);
            }
            Entry::Vacant(entry) => {
                entry.insert(ack.clone());
            }
        }
    }
}

/// Bitset of previous ticks that were successfully retrieved.
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
    pub fn set_base(&mut self, new_base: NetworkTick) -> Vec<NetworkTick> {
        let mut unacked = Vec::new();

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
