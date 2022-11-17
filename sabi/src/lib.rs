// Re-export the derive macros here.
#[allow(unused_imports)]
#[macro_use]
#[cfg(feature = "public")]
extern crate sabi_derive;

#[doc(hidden)]
#[cfg(feature = "public")]
pub use sabi_derive::*;
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

#[derive(Default, Debug, Clone, Copy)]
pub struct Client;

#[derive(Default, Debug, Clone, Copy)]
pub struct Server;

#[derive(Default, Debug, Clone, Copy, Resource)]
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
    pub use crate::replicate::{Replicate, ReplicateId};
}

#[cfg(feature = "public")]
pub use crate::replicate::{Replicate, ReplicateId};
