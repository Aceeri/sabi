use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

use serde::{Deserialize, Serialize};

mod general;
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
