use bevy::prelude::*;

use crate::prelude::Replicate;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Replicate)]
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
}
