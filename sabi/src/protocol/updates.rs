use bevy::{
    ecs::entity::Entities,
    prelude::*,
    reflect::{FromReflect, TypeRegistry},
    render::camera::{ActiveCamera, Camera3d},
    utils::{Entry, HashMap},
};
use bevy_renet::{
    renet::{
        BlockChannelConfig, ChannelConfig, ConnectToken, ReliableChannelConfig, RenetClient,
        RenetConnectionConfig, RenetError, RenetServer, ServerConfig, ServerEvent,
        UnreliableChannelConfig, NETCODE_KEY_BYTES,
    },
    run_if_client_conected, RenetClientPlugin, RenetServerPlugin,
};
use iyes_loopless::prelude::*;

use std::{
    hash::{Hash, Hasher},
    net::{Ipv4Addr, SocketAddrV4, ToSocketAddrs, UdpSocket},
    time::Duration,
};

use serde::{Deserialize, Serialize};

use std::time::SystemTime;

use std::f32::consts::TAU;

use bevy_rapier3d::prelude::*;

use crate::{prelude::*, replicate::physics::ReplicatePhysicsPlugin};

#[derive(Debug, Deref, DerefMut, Clone, Serialize, Deserialize)]
pub struct EntityUpdate(pub HashMap<ServerEntity, ComponentsUpdate>);

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
