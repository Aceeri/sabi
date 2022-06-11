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

#[derive(Default, Deref, DerefMut, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentsUpdate(pub HashMap<ReplicateId, Vec<u8>>);

impl ComponentsUpdate {
    pub fn new() -> Self {
        Self(HashMap::new())
    }
}

impl EntityUpdate {
    pub fn protocol_id() -> u64 {
        1
    }
}
