use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

use serde::{Deserialize, Serialize};

mod physics;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ReplicateId(pub u32);

pub trait Replicate
where
    Self: Sized,
{
    type Def: Serialize + for<'de> Deserialize<'de>;
    fn into_def(self) -> Self::Def;
    fn apply_def(&mut self, def: Self::Def) {
        *self = Self::from_def(def);
    }
    fn from_def(def: Self::Def) -> Self;
    fn replicate_id() -> ReplicateId;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(remote = "Transform")]
pub struct TransformDef {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicateTransform(#[serde(with = "TransformDef")] pub Transform);

impl Replicate for Transform {
    type Def = ReplicateTransform;
    fn into_def(self) -> Self::Def {
        ReplicateTransform(self)
    }
    fn from_def(def: Self::Def) -> Self {
        def.0
    }
    fn replicate_id() -> ReplicateId {
        ReplicateId(1)
    }
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
