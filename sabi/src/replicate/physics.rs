use bevy::prelude::*;
use bevy_rapier3d::{prelude::*, rapier::prelude::SharedShape};

use serde::{Deserialize, Serialize};

use crate::{plugin::ReplicatePlugin, Replicate};

pub struct ReplicatePhysicsPlugin;
impl Plugin for ReplicatePhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(ReplicatePlugin::<RigidBody>::default());
        app.add_plugin(ReplicatePlugin::<Velocity>::default());
        app.add_plugin(ReplicatePlugin::<MassProperties>::default());
        app.add_plugin(ReplicatePlugin::<AdditionalMassProperties>::default());
        app.add_plugin(ReplicatePlugin::<LockedAxes>::default());
        app.add_plugin(ReplicatePlugin::<ExternalForce>::default());
        app.add_plugin(ReplicatePlugin::<ExternalImpulse>::default());
        app.add_plugin(ReplicatePlugin::<Ccd>::default());
        app.add_plugin(ReplicatePlugin::<Sleeping>::default());
        app.add_plugin(ReplicatePlugin::<Dominance>::default());
        app.add_plugin(ReplicatePlugin::<Damping>::default());
        app.add_plugin(ReplicatePlugin::<Restitution>::default());
        app.add_plugin(ReplicatePlugin::<Friction>::default());
        app.add_plugin(ReplicatePlugin::<GravityScale>::default());
        app.add_plugin(ReplicatePlugin::<Sensor>::default());
        app.add_plugin(ReplicatePlugin::<CollisionGroups>::default());
        app.add_plugin(ReplicatePlugin::<SolverGroups>::default());
        app.add_plugin(ReplicatePlugin::<Collider>::default());
        app.add_plugin(ReplicatePlugin::<ColliderScale>::default());
    }
}

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

#[derive(Debug, Clone, Serialize, Deserialize, Replicate)]
#[serde(remote = "MassProperties")]
#[replicate(remote = "MassProperties")]
#[replicate(crate = "crate")]
pub struct MassPropertiesDef {
    pub local_center_of_mass: Vec3,
    pub mass: f32,
    pub principal_inertia_local_frame: Quat,
    pub principal_inertia: Vec3,
}

#[derive(Debug, Clone, Serialize, Deserialize, Replicate)]
#[serde(remote = "AdditionalMassProperties")]
#[replicate(remote = "AdditionalMassProperties")]
#[replicate(crate = "crate")]
pub struct AdditionalMassPropertiesDef(#[serde(with = "MassPropertiesDef")] pub MassProperties);

impl Replicate for LockedAxes {
    type Def = u8;
    fn into_def(self) -> Self::Def {
        self.bits()
    }
    fn from_def(def: Self::Def) -> Self {
        LockedAxes::from_bits(def).expect("locked axes from bits")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Replicate)]
#[serde(remote = "ExternalForce")]
#[replicate(remote = "ExternalForce")]
#[replicate(crate = "crate")]
pub struct ExternalForceDef {
    pub force: Vec3,
    pub torque: Vec3,
}

#[derive(Debug, Clone, Serialize, Deserialize, Replicate)]
#[serde(remote = "ExternalImpulse")]
#[replicate(remote = "ExternalImpulse")]
#[replicate(crate = "crate")]
pub struct ExternalImpulseDef {
    pub impulse: Vec3,
    pub torque_impulse: Vec3,
}

#[derive(Debug, Clone, Serialize, Deserialize, Replicate)]
#[serde(remote = "Sleeping")]
#[replicate(remote = "Sleeping")]
#[replicate(crate = "crate")]
pub struct SleepingDef {
    pub linear_threshold: f32,
    pub angular_threshold: f32,
    pub sleeping: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Replicate)]
#[serde(remote = "Damping")]
#[replicate(remote = "Damping")]
#[replicate(crate = "crate")]
pub struct DampingDef {
    pub linear_damping: f32,
    pub angular_damping: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(remote = "CoefficientCombineRule")]
pub enum CoefficientCombineRuleDef {
    Average,
    Min,
    Multiply,
    Max,
}

#[derive(Debug, Clone, Serialize, Deserialize, Replicate)]
#[serde(remote = "Friction")]
#[replicate(remote = "Friction")]
#[replicate(crate = "crate")]
pub struct FrictionDef {
    pub coefficient: f32,

    #[serde(with = "CoefficientCombineRuleDef")]
    pub combine_rule: CoefficientCombineRule,
}

#[derive(Debug, Clone, Serialize, Deserialize, Replicate)]
#[serde(remote = "Restitution")]
#[replicate(remote = "Restitution")]
#[replicate(crate = "crate")]
pub struct RestitutionDef {
    pub coefficient: f32,

    #[serde(with = "CoefficientCombineRuleDef")]
    pub combine_rule: CoefficientCombineRule,
}

impl Replicate for Ccd {
    type Def = bool;
    fn into_def(self) -> Self::Def {
        self.enabled
    }
    fn from_def(def: Self::Def) -> Self {
        Ccd { enabled: def }
    }
}

impl Replicate for Sensor {
    type Def = bool;
    fn into_def(self) -> Self::Def {
        self.0
    }
    fn from_def(def: Self::Def) -> Self {
        Sensor(def)
    }
}
impl Replicate for GravityScale {
    type Def = f32;
    fn into_def(self) -> Self::Def {
        self.0
    }
    fn from_def(def: Self::Def) -> Self {
        GravityScale(def)
    }
}

impl Replicate for Dominance {
    type Def = i8;
    fn into_def(self) -> Self::Def {
        self.groups
    }
    fn from_def(def: Self::Def) -> Self {
        Dominance { groups: def }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Replicate)]
#[serde(remote = "CollisionGroups")]
#[replicate(remote = "CollisionGroups")]
#[replicate(crate = "crate")]
pub struct CollisionGroupsDef {
    pub memberships: u32,
    pub filters: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Replicate)]
#[serde(remote = "SolverGroups")]
#[replicate(remote = "SolverGroups")]
#[replicate(crate = "crate")]
pub struct SolverGroupsDef {
    pub memberships: u32,
    pub filters: u32,
}

impl Replicate for Collider {
    type Def = SharedShape;
    fn into_def(self) -> Self::Def {
        self.raw
    }
    fn from_def(shared_shape: Self::Def) -> Self {
        Collider::from(shared_shape)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Replicate)]
#[serde(remote = "ColliderScale")]
#[replicate(remote = "ColliderScale")]
#[replicate(crate = "crate")]
pub enum ColliderScaleDef {
    Relative(Vec3),
    Absolute(Vec3),
}
