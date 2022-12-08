use bevy::prelude::*;

pub mod error;
pub mod lobby;
#[cfg(feature = "public")]
pub mod message_sample;
pub mod plugin;
#[cfg(feature = "public")]
pub mod protocol;
#[cfg(feature = "public")]
pub mod replicate;
pub mod stage;
pub mod tick;

/// Marker resource to denote that this should receive replication information.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct Client;

/// Marker resource to denote that this should receive inputs and send replication
/// information.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct Server;

/// Act as both server and client.
///
/// Effectively this should just do nothing in terms of networking.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct Local;

pub mod prelude {
    #[cfg(feature = "public")]
    pub use crate::protocol::{
        ClientChannel, Owned, ServerChannel, ServerEntities, ServerEntity, ServerMessage,
    };

    pub use crate::error::SabiError;
    pub use crate::lobby::{ClientId, Lobby};
    pub use crate::tick::{tick_hz, NetworkTick};

    #[cfg(feature = "public")]
    pub use crate::plugin::{ReplicatePlugin, SabiPlugin};
    #[cfg(feature = "public")]
    pub use crate::replicate::{replicate_id, ReplicateId};
}

#[cfg(feature = "public")]
pub use crate::replicate::{replicate_id, ReplicateId};
