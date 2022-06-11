use std::fmt;

use bevy::{prelude::*, utils::HashMap};

use crate::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Deref, DerefMut, Clone, Serialize, Deserialize)]
pub struct ClientEntityUpdate(pub HashMap<u64, EntityUpdate>);

#[derive(Deref, DerefMut, Clone, Serialize, Deserialize)]
pub struct EntityUpdate(pub HashMap<ServerEntity, ComponentsUpdate>);

impl fmt::Debug for EntityUpdate {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut counts: HashMap<ReplicateId, u16> = HashMap::new();

        for (_, component_update) in self.iter() {
            for (replicate_id, _) in component_update.iter() {
                *counts.entry(*replicate_id).or_insert(0) += 1;
            }
        }

        f.debug_struct("EntityUpdate")
            .field("entities", &self.0.len())
            .field("components", &counts)
            .finish()
    }
}

impl EntityUpdate {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }
}

/// Thin wrapper type to differentiate between reliable and unreliable components.
///
/// Reliable means that we probably aren't going to be updating it that often and we don't care as much about the latency.
/// Unreliable means that we are probably updating it extremely frequently.
#[derive(Debug, Deref, DerefMut, Clone, Serialize, Deserialize)]
pub struct Reliable<T>(pub T);

#[derive(Debug, Deref, DerefMut, Clone, Serialize, Deserialize)]
pub struct Unreliable<T>(pub T);

#[derive(Default, Deref, DerefMut, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentsUpdate(pub HashMap<ReplicateId, Vec<u8>>);

impl ComponentsUpdate {
    pub fn new() -> Self {
        Self(HashMap::new())
    }
}

impl Reliable<EntityUpdate> {
    pub fn protocol_id() -> u64 {
        1
    }
}

impl Unreliable<EntityUpdate> {
    pub fn protocol_id() -> u64 {
        1
    }
}
