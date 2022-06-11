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

pub fn client_recv_interest_reliable(
    mut commands: Commands,
    mut server_entities: ResMut<ServerEntities>,
    mut update_events: EventWriter<(ServerEntity, ComponentsUpdate)>,
    mut client: ResMut<RenetClient>,
) {
    while let Some(message) = client.receive_message(COMPONENT) {
        let decompressed = zstd::bulk::decompress(&message.as_slice(), 10 * 1024).unwrap();
        let data: EntityUpdate = bincode::deserialize(&decompressed).unwrap();

        for (server_entity, _) in data.iter() {
            server_entities.spawn_or_get(&mut commands, *server_entity);
        }

        update_events.send_batch(data.0.into_iter());
    }
}

pub fn client_update_reliable<C>(
    mut commands: Commands,
    mut server_entities: ResMut<ServerEntities>,
    mut update_events: EventReader<(ServerEntity, ComponentsUpdate)>,
    mut query: Query<&mut C>,
) where
    C: 'static + Send + Sync + Component + Replicate,
{
    for (server_entity, components_update) in update_events.iter() {
        if let Some(update_data) = components_update.get(&C::replicate_id()) {
            let def: <C as Replicate>::Def = bincode::deserialize(&update_data).unwrap();
            let entity = server_entities.spawn_or_get(&mut commands, *server_entity);

            if let Ok(mut component) = query.get_mut(entity) {
                component.apply_def(def);
            } else {
                let component = C::from_def(def);
                commands.entity(entity).insert(component);
            }
        }
    }
}
