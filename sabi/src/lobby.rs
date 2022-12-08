use bevy::prelude::*;
use bevy::utils::HashMap;

pub type ClientId = u64;

/// Renet Client ID -> Player Character Entity mapping
#[derive(Resource, Debug, Default)]
pub struct Lobby {
    pub players: HashMap<ClientId, Entity>,
}
