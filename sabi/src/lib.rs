// Re-export the derive macros here.
#[allow(unused_imports)]
#[macro_use]
extern crate sabi_derive;

#[doc(hidden)]
pub use sabi_derive::*;

pub mod error;
pub mod message_sample;
pub mod plugin;
pub mod protocol;
pub mod replicate;
pub mod stage;

#[derive(Default, Debug, Clone, Copy)]
pub struct Client;

#[derive(Default, Debug, Clone, Copy)]
pub struct Server;

#[derive(Default, Debug, Clone, Copy)]
pub struct Local;

pub mod prelude {
    pub use crate::protocol::{
        lobby::Lobby, tick_hz, ClientChannel, Owned, ServerChannel, ServerEntities, ServerEntity,
        ServerMessage,
    };

    pub use crate::error::SabiError;
    pub use crate::plugin::{ReplicatePlugin, SabiPlugin};
    pub use crate::replicate::{Replicate, ReplicateId};
}

pub use crate::replicate::{Replicate, ReplicateId};
