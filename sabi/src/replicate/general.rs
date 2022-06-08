use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

use crate::prelude::{Replicate, ReplicateId};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Replicate)]
#[serde(remote = "Transform")]
#[replicate(remote = "Transform")]
#[replicate(crate = "crate")]
pub struct TransformDef {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Replicate for Name {
    type Def = String;
    fn into_def(self) -> Self::Def {
        self.as_str().to_owned()
    }
    fn apply_def(&mut self, def: Self::Def) {
        self.set(def);
    }
    fn from_def(def: Self::Def) -> Self {
        Name::new(def)
    }
    fn replicate_id() -> ReplicateId {
        ReplicateId(3)
    }
}
