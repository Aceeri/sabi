use bevy::math::Vec3;
use bevy::prelude::*;
use bevy_rapier3d::{
    prelude::Collider,
    rapier::prelude::{SharedShape, TypedShape},
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::Replicate;
