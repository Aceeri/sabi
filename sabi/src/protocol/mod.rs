use bevy::{
    ecs::entity::Entities,
    prelude::*,
    reflect::{FromReflect},
    utils::{Entry, HashMap},
};
use bevy_renet::{
    renet::{
        BlockChannelConfig, ChannelConfig, ReliableChannelConfig,
        RenetConnectionConfig,
        UnreliableChannelConfig, NETCODE_KEY_BYTES,
    },
};


use std::{
    hash::{Hash, Hasher},
    net::{UdpSocket},
    time::Duration,
};

use serde::{Deserialize, Serialize};







use crate::{prelude::*};

pub mod client;
pub mod server;
pub mod updates;

pub use client::*;
pub use server::*;
pub use updates::{ComponentsUpdate, EntityUpdate, Reliable, Unreliable};

/// Private key for signing connect tokens for clients.
///
/// This should be changed and not generated in the code here, instead used via a
/// matchmaking server/relay.
pub const PRIVATE_KEY: &[u8; NETCODE_KEY_BYTES] = b"JKS$C14tDvez8trgbdZcIuU&wz#OjG&3"; // 32-bytes
pub const PORT: u16 = 42069;

/// Channel IDs
pub const SERVER_RELIABLE: u8 = 0;
pub const UNRELIABLE: u8 = 1;
pub const BLOCK: u8 = 2;
pub const COMPONENT_RELIABLE: u8 = 3;

/// If we see this component we have control over this entity.
///
/// The server should have `Owned` on most things while the client should have it on just a few.
/// Mainly so the client can predict things like their character moving.
#[derive(Debug, Deserialize, Component, Reflect)]
pub struct Owned;

/// Renet Client ID -> Player Character Entity mapping
#[derive(Debug, Default)]
pub struct Lobby {
    pub players: HashMap<u64, Entity>,
}

/// Reliable protocol from the server to the clients for communicating the
/// overall gamestate and assigning what the clients should predict.
#[derive(Debug, Serialize, Deserialize, Component)]
pub enum ServerMessage {
    SetPlayer { id: u64, entity: ServerEntity },
    AssignOwnership { entity: ServerEntity },
    PlayerConnected { id: u64, entity: ServerEntity },
    PlayerDisconnected { id: u64 },
}

impl ServerMessage {
    pub fn protocol_id() -> u64 {
        1
    }
}

/// A unique identifier that is used to refer to entities across:
/// server and client boundaries.
///
/// In this case it is *literally* just the server's `Entity`,
/// since that will give us generational indexing without us doing much.
///
/// This won't work for P2P kinds of games but for our case its fine.
#[derive(
    Default,
    Debug,
    Clone,
    Copy,
    Component,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Reflect,
    FromReflect,
    Serialize,
    Deserialize,
)]
#[reflect(Component, Hash, PartialEq)]
pub struct ServerEntity(u32, u32);

impl ServerEntity {
    pub fn from_entity(entity: Entity) -> Self {
        Self(entity.id(), entity.generation())
    }
}

impl From<Entity> for ServerEntity {
    fn from(entity: Entity) -> Self {
        Self::from_entity(entity)
    }
}

/// Authoritative mapping of server entities to entities for clients.
///
/// This is so clients can figure out which entity the server is talking about.
#[derive(Default, Debug, Clone)]
pub struct ServerEntities(HashMap<ServerEntity, Entity>);

impl ServerEntities {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn spawn_or_get(&mut self, commands: &mut Commands, server_entity: ServerEntity) -> Entity {
        match self.0.entry(server_entity) {
            Entry::Occupied(entity) => *entity.get(),
            Entry::Vacant(vacant) => {
                let new_entity = commands.spawn().insert(server_entity).id();
                vacant.insert(new_entity);
                new_entity
            }
        }
    }

    pub fn get(&self, entities: &Entities, server_entity: ServerEntity) -> Option<Entity> {
        let entity = self.0.get(&server_entity).cloned();
        entity.filter(|entity| entities.contains(*entity))
    }

    pub fn clean(&mut self, entities: &Entities) -> bool {
        let mut dead = Vec::new();
        for (server_entity, entity) in self.0.iter() {
            if !entities.contains(*entity) {
                dead.push(*server_entity);
            }
        }

        for server_entity in dead.iter() {
            self.0.remove(server_entity);
        }

        dead.len() > 0
    }
}

/// Local ip to bind to so we can have others connect.
///
/// Windows is weird here and doesn't let you do it on `localhost`/`127.0.0.1`
///
/// So instead we have a `0.0.0.0` address which is kind of like saying we'll take
/// any address.
pub fn localhost_ip() -> &'static str {
    #[cfg(target_family = "windows")]
    return "0.0.0.0";
    #[cfg(target_family = "unix")]
    return "127.0.0.1";
}

/// Protocol identifier so we have more obvious breakage when we change the protocol.
pub fn protocol_id() -> u64 {
    let concat = format!(
        "server:{};unreliable:{};reliable:{}",
        ServerMessage::protocol_id().to_string(),
        Reliable::<EntityUpdate>::protocol_id().to_string(),
        Unreliable::<EntityUpdate>::protocol_id().to_string(),
    );
    let mut s = std::collections::hash_map::DefaultHasher::new();
    concat.hash(&mut s);
    s.finish()
}

pub fn renet_connection_config() -> RenetConnectionConfig {
    let mut connection_config = RenetConnectionConfig::default();
    connection_config.channels_config = vec![
        ChannelConfig::Reliable(ReliableChannelConfig {
            channel_id: 0,
            ..Default::default()
        }),
        ChannelConfig::Unreliable(UnreliableChannelConfig {
            channel_id: 1,
            ..Default::default()
        }),
        ChannelConfig::Block(BlockChannelConfig {
            channel_id: 2,
            ..Default::default()
        }),
        ChannelConfig::Reliable(ReliableChannelConfig {
            channel_id: 3,
            ..Default::default()
        }),
    ];

    connection_config
}

/// Fetch our external ip using Cloudflare's DNS resolver.
///
/// We need this for verifying that connections from clients are valid
pub fn public_ip() -> Option<String> {
    let socket = match UdpSocket::bind((localhost_ip(), 0)) {
        Ok(s) => s,
        Err(_) => return None,
    };

    match socket.connect("1.1.1.1:80") {
        Ok(()) => (),
        Err(_) => return None,
    };

    match socket.local_addr() {
        Ok(addr) => return Some(addr.ip().to_string()),
        Err(_) => return None,
    };
}

/// Tick rate of the network sending/receiving
#[derive(Deref, DerefMut, Debug, Clone)]
pub struct NetworkGameTimer(pub Timer);

impl Default for NetworkGameTimer {
    fn default() -> Self {
        // 16Hz
        Self(Timer::new(Duration::from_micros(15625 * 4), true))
    }
}

pub fn tick_network(time: Res<Time>, mut network_timer: ResMut<NetworkGameTimer>) {
    network_timer.tick(time.delta());
}

pub fn on_network_tick(network_timer: Res<NetworkGameTimer>) -> bool {
    network_timer.just_finished()
}
