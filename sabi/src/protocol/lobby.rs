use super::ClientId;
use bevy::prelude::*;
use bevy::utils::HashMap;

/// Renet Client ID -> Player Character Entity mapping
#[derive(Debug, Default)]
pub struct Lobby {
    pub players: HashMap<ClientId, Entity>,
}
