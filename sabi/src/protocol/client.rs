use bevy::prelude::*;
use bevy_renet::renet::{ConnectToken, RenetClient};

use std::net::{ToSocketAddrs, UdpSocket};

use std::time::SystemTime;

use crate::protocol::*;

pub fn new_renet_client() -> RenetClient {
    let server_ip = my_internet_ip::get().unwrap();
    let server_addr = format!("{}:{}", server_ip, crate::protocol::PORT)
        .to_socket_addrs()
        .unwrap()
        .next()
        .unwrap();

    println!("server addr: {:?}", server_addr);
    let protocol_id = protocol_id();
    println!("protocol id: {:?}", protocol_id);

    let connection_config = renet_connection_config();
    let socket = UdpSocket::bind((localhost_ip(), 0)).unwrap();
    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let client_id = current_time.as_millis() as u64;

    // This connect token should come from another system, NOT generated from the client.
    // Usually from a matchmaking system
    // The client should not have access to the PRIVATE_KEY from the server.
    let token = ConnectToken::generate(
        current_time,
        protocol_id,
        300,
        client_id,
        15,
        vec![server_addr],
        None,
        PRIVATE_KEY,
    )
    .unwrap();
    RenetClient::new(current_time, socket, client_id, token, connection_config).unwrap()
}

pub fn client_connected(client: ResMut<RenetClient>) -> bool {
    client.is_connected()
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

    /// Despawn any server entities
    pub fn disconnect(&mut self, entities: &Entities, commands: &mut Commands) {
        for (_server_entity, entity) in self.0.drain() {
            if entities.contains(entity) {
                commands.entity(entity).despawn_recursive();
            }
        }
    }
}
