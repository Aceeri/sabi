use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use bevy::prelude::*;
use std::marker::PhantomData;

use serde::{Deserialize, Serialize};

pub mod collider;
pub mod general;
pub mod physics;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ReplicateId(pub u64);

pub trait Replicate
where
    Self: Sized,
{
    type Def: Serialize + for<'de> Deserialize<'de>;
    fn into_def(self) -> Self::Def;
    fn from_def(def: Self::Def) -> Self;
    fn apply_def(&mut self, def: Self::Def) {
        *self = Self::from_def(def);
    }
    fn replicate_id() -> ReplicateId {
        let mut hasher = DefaultHasher::new();
        std::any::type_name::<Self>().hash(&mut hasher);
        let wide = hasher.finish(); // prob slim this down in the future
        ReplicateId(wide)
    }
}

pub enum ReplicationMark<C>
where
    C: 'static + Component + Replicate,
{
    /// Make sure the component gets to the client once so it knows to have it on the entity.
    ///
    /// Past that never re-sends.
    Once(PhantomData<C>),
    /// Sends a component whenever it is highest in priority.
    Constant(PhantomData<C>),
}
