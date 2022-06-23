use bevy::{
    ecs::entity::Entities,
    prelude::*,
    reflect::FromReflect,
    utils::{Entry, HashMap},
};
use bevy_renet::renet::{
    BlockChannelConfig, ChannelConfig, ReliableChannelConfig, RenetConnectionConfig,
    UnreliableChannelConfig, NETCODE_KEY_BYTES,
};

use std::{
    hash::{Hash, Hasher},
    net::UdpSocket,
};

use serde::{Deserialize, Serialize};

use crate::prelude::*;

pub mod client;
pub mod input;
pub mod lobby;
pub mod priority;
pub mod resim;
pub mod server;
pub mod tick;
pub mod update;

pub use client::*;
pub use server::*;
pub use tick::{on_network_tick, tick_hz, tick_network, NetworkGameTimer, NetworkTick};
pub use update::{ComponentsUpdate, EntityUpdate};

/// Private key for signing connect tokens for clients.
///
/// This should be changed and not generated in the code here, instead used via a
/// matchmaking server/relay.
pub const PRIVATE_KEY: &[u8; NETCODE_KEY_BYTES] = b"JKS$C14tDvez8trgbdZcIuU&wz#OjG&3"; // 32-bytes
pub const PORT: u16 = 42069;

pub type ClientId = u64;

pub mod channel {
    /// Channel IDs
    pub const SERVER_MESSAGE: u8 = 0;
    pub const CLIENT_INPUT: u8 = 1;
    pub const COMPONENT: u8 = 2;
    pub const BLOCK: u8 = 3;
}

/// If we see this component we have control over this entity.
///
/// The server should have `Owned` on most things while the client should have it on just a few.
/// Mainly so the client can predict things like their character moving.
#[derive(Debug, Deserialize, Component, Reflect)]
pub struct Owned;

/// Reliable protocol from the server to the clients for communicating the
/// overall gamestate and assigning what the clients should predict.
#[derive(Debug, Serialize, Deserialize, Component)]
pub enum ServerMessage {
    SetPlayer { id: ClientId, entity: ServerEntity },
    AssignOwnership { entity: ServerEntity },
    PlayerConnected { id: ClientId, entity: ServerEntity },
    PlayerDisconnected { id: ClientId },
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
        "server:{};entity:{};",
        ServerMessage::protocol_id().to_string(),
        EntityUpdate::protocol_id().to_string(),
    );
    let mut s = std::collections::hash_map::DefaultHasher::new();
    concat.hash(&mut s);
    s.finish()
}

pub fn renet_connection_config() -> RenetConnectionConfig {
    let mut connection_config = RenetConnectionConfig::default();
    connection_config.channels_config = vec![
        ChannelConfig::Reliable(ReliableChannelConfig {
            channel_id: channel::SERVER_MESSAGE,
            ..Default::default()
        }),
        ChannelConfig::Unreliable(UnreliableChannelConfig {
            channel_id: channel::CLIENT_INPUT,
            ..Default::default()
        }),
        ChannelConfig::Unreliable(UnreliableChannelConfig {
            channel_id: channel::COMPONENT,
            ..Default::default()
        }),
        ChannelConfig::Block(BlockChannelConfig {
            channel_id: channel::BLOCK,
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
