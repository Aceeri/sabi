// Re-export the derive macros here.
#[allow(unused_imports)]
#[macro_use]
extern crate sabi_derive;

#[doc(hidden)]
pub use sabi_derive::*;

pub mod error;
pub mod plugin;
pub mod protocol;
pub mod replicate;
pub mod message_sample;
pub mod stage;

#[derive(Debug, Clone, Copy)]
pub struct Client;

#[derive(Debug, Clone, Copy)]
pub struct Server;

pub mod prelude {
    pub use crate::protocol::{
        channel, lobby::Lobby, tick_hz, Owned, ServerEntities, ServerEntity, ServerMessage,
    };

    pub use crate::error::SabiError;
    pub use crate::plugin::{ReplicatePlugin, SabiPlugin};
    pub use crate::replicate::{Replicate, ReplicateId};
}

pub use crate::replicate::{Replicate, ReplicateId};
