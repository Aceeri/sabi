// Re-export the derive macros here.
#[allow(unused_imports)]
#[macro_use]
extern crate sabi_derive;

#[doc(hidden)]
pub use sabi_derive::*;

pub mod protocol;
pub mod replicate;

pub mod prelude {
    pub use crate::protocol::{
        Owned, SabiClientPlugin, SabiServerPlugin, ServerEntities, ServerEntity, ServerMessage,
    };

    pub use crate::replicate::{Replicate, ReplicateId};
}

pub use crate::replicate::{Replicate, ReplicateId};
