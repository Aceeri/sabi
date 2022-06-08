use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

use serde::{Deserialize, Serialize};

use crate::{Replicate, ReplicateId};

#[derive(Debug, Clone, Serialize, Deserialize, Replicate)]
#[serde(remote = "RigidBody")]
#[replicate(remote = "RigidBody")]
#[replicate(crate = "crate")]
pub enum RigidBodyDef {
    Dynamic,
    Fixed,
    KinematicVelocityBased,
    KinematicPositionBased,
}

#[derive(Debug, Clone, Serialize, Deserialize, Replicate)]
#[serde(remote = "Velocity")]
#[replicate(remote = "Velocity")]
#[replicate(crate = "crate")]
pub struct VelocityDef {
    pub linvel: Vec3,
    pub angvel: Vec3,
}
