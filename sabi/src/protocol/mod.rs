use bevy::{
    ecs::entity::Entities,
    prelude::*,
    reflect::{FromReflect, TypeRegistry},
    render::camera::{ActiveCamera, Camera3d},
    utils::{Entry, HashMap},
};
use bevy_renet::{
    renet::{
        BlockChannelConfig, ChannelConfig, ConnectToken, ReliableChannelConfig, RenetClient,
        RenetConnectionConfig, RenetError, RenetServer, ServerConfig, ServerEvent,
        UnreliableChannelConfig, NETCODE_KEY_BYTES,
    },
    run_if_client_conected, RenetClientPlugin, RenetServerPlugin,
};
use iyes_loopless::prelude::*;

use std::{
    hash::{Hash, Hasher},
    net::{Ipv4Addr, SocketAddrV4, ToSocketAddrs, UdpSocket},
    time::Duration,
};

use serde::{Deserialize, Serialize};

use std::time::SystemTime;

use std::f32::consts::TAU;

use bevy_rapier3d::prelude::*;

use crate::prelude::*;

const PRIVATE_KEY: &[u8; NETCODE_KEY_BYTES] = b"JKS$C14tDvez8trgbdZcIuU&wz#OjG&3"; // 32-bytes
const PORT: u16 = 42069;

/// Channel IDs
pub const SERVER_RELIABLE: u8 = 0;
pub const UNRELIABLE: u8 = 1;
pub const BLOCK: u8 = 2;
pub const COMPONENT_RELIABLE: u8 = 3;

#[derive(Debug, Deserialize, Component, Reflect)]
pub struct Owned;

#[derive(Debug, Default)]
pub struct Lobby {
    pub players: HashMap<u64, Entity>,
}

#[derive(Debug, Serialize, Deserialize, Component)]
pub enum ServerMessage {
    SetPlayer { id: u64, entity: ServerEntity },
    AssignOwnership { entity: ServerEntity },
    PlayerConnected { id: u64, entity: ServerEntity },
    PlayerDisconnected { id: u64 },
}

impl ServerMessage {
    pub fn protocol_id() -> u64 {
        1
    }
}

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

#[derive(Debug, Deref, DerefMut, Clone, Serialize, Deserialize)]
pub struct EntityUpdate(HashMap<ServerEntity, ComponentsUpdate>);

impl EntityUpdate {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }
}

/// Thin wrapper type to differentiate between reliable and unreliable components.
///
/// Reliable means that we probably aren't going to be updating it that often and we don't care as much about the latency.
/// Unreliable means that we are probably updating it extremely frequently.
#[derive(Debug, Deref, DerefMut, Clone, Serialize, Deserialize)]
pub struct Reliable<T>(pub T);

#[derive(Debug, Deref, DerefMut, Clone, Serialize, Deserialize)]
pub struct Unreliable<T>(pub T);

#[derive(Default, Deref, DerefMut, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentsUpdate(HashMap<ReplicateId, Vec<u8>>);

impl ComponentsUpdate {
    pub fn new() -> Self {
        Self(HashMap::new())
    }
}

pub fn server_clear_reliable_queue(mut updates: ResMut<Reliable<EntityUpdate>>) {
    updates.clear();
}

pub fn server_queue_interest_reliable<C>(
    mut updates: ResMut<Reliable<EntityUpdate>>,
    query: Query<(Entity, &C)>,
) where
    C: 'static + Send + Sync + Component + Replicate + Clone,
{
    for (entity, component) in query.iter() {
        let server_entity = ServerEntity::from_entity(entity);
        let component_def = component.clone().into_def();
        let component_data = bincode::serialize(&component_def).unwrap();
        let update = updates
            .entry(server_entity)
            .or_insert(ComponentsUpdate::new());
        update.insert(C::replicate_id(), component_data);
    }
}

pub fn server_send_interest_reliable(
    updates: Res<Reliable<EntityUpdate>>,
    mut server: ResMut<RenetServer>,
) {
    let data = bincode::serialize(&*updates).unwrap();
    let data = zstd::bulk::compress(&data.as_slice(), 0).unwrap();
    server.broadcast_message(COMPONENT_RELIABLE, data);
}

pub fn client_recv_interest_reliable(
    mut commands: Commands,
    mut server_entities: ResMut<ServerEntities>,
    mut update_events: EventWriter<(ServerEntity, ComponentsUpdate)>,
    mut client: ResMut<RenetClient>,
) {
    while let Some(message) = client.receive_message(COMPONENT_RELIABLE) {
        let decompressed = zstd::bulk::decompress(&message.as_slice(), 1024).unwrap();
        let data: Reliable<EntityUpdate> = bincode::deserialize(&decompressed).unwrap();

        for (server_entity, _) in data.iter() {
            server_entities.spawn_or_get(&mut commands, *server_entity);
        }

        update_events.send_batch(data.0 .0.into_iter());
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
}

impl Reliable<EntityUpdate> {
    pub fn protocol_id() -> u64 {
        1
    }
}

impl Unreliable<EntityUpdate> {
    pub fn protocol_id() -> u64 {
        1
    }
}

pub fn protocol_id() -> u64 {
    let concat = format!(
        "server:{};unreliable:{};reliable:{}",
        ServerMessage::protocol_id().to_string(),
        Reliable::<EntityUpdate>::protocol_id().to_string(),
        Unreliable::<EntityUpdate>::protocol_id().to_string(),
    );
    let mut s = std::collections::hash_map::DefaultHasher::new();
    concat.hash(&mut s);
    s.finish()
}

fn localhost_addr() -> &'static str {
    #[cfg(target_family = "windows")]
    return "0.0.0.0";
    #[cfg(target_family = "unix")]
    return "127.0.0.1";
}

fn renet_connection_config() -> RenetConnectionConfig {
    let mut connection_config = RenetConnectionConfig::default();
    connection_config.channels_config = vec![
        ChannelConfig::Reliable(ReliableChannelConfig {
            channel_id: 0,
            ..Default::default()
        }),
        ChannelConfig::Unreliable(UnreliableChannelConfig {
            channel_id: 1,
            ..Default::default()
        }),
        ChannelConfig::Block(BlockChannelConfig {
            channel_id: 2,
            ..Default::default()
        }),
        ChannelConfig::Reliable(ReliableChannelConfig {
            channel_id: 3,
            ..Default::default()
        }),
    ];

    connection_config
}

fn new_renet_client() -> RenetClient {
    let server_ip = my_internet_ip::get().unwrap();
    //let server_ip = "spite.aceeri.com";
    //let server_ip = "72.134.64.107";
    let server_addr = format!("{}:{}", server_ip, PORT)
        .to_socket_addrs()
        .unwrap()
        .next()
        .unwrap();

    println!("server addr: {:?}", server_addr);
    let protocol_id = protocol_id();
    println!("protocol id: {:?}", protocol_id);

    let connection_config = renet_connection_config();
    let socket = UdpSocket::bind((localhost_addr(), 0)).unwrap();
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

fn get_local_ipaddress() -> Option<String> {
    let socket = match UdpSocket::bind((localhost_addr(), 0)) {
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

fn new_renet_server() -> RenetServer {
    let local_ip = get_local_ipaddress().unwrap_or(localhost_addr().to_owned());
    let port = 42069;

    match igd::search_gateway(igd::SearchOptions {
        timeout: Some(Duration::from_secs(1)),
        ..Default::default()
    }) {
        Err(ref err) => println!("Error: {}", err),
        Ok(gateway) => {
            let local_addr = local_ip.parse::<Ipv4Addr>().unwrap();
            let local_addr = SocketAddrV4::new(local_addr, port);

            match gateway.add_port(
                igd::PortMappingProtocol::UDP,
                port,
                local_addr,
                0,
                "add_port example",
            ) {
                Err(ref err) => {
                    println!("There was an error! {}", err);
                }
                Ok(()) => {
                    println!("It worked");
                }
            }
        }
    }

    let server_ip = my_internet_ip::get().unwrap();
    let server_addr = format!("{}:{}", server_ip, PORT)
        .to_socket_addrs()
        .unwrap()
        .next()
        .unwrap();

    let local_addr = format!("{}:{}", local_ip, PORT)
        .to_socket_addrs()
        .unwrap()
        .next()
        .unwrap();

    println!("binding to {:?}", server_addr);
    let protocol_id = protocol_id();
    println!("protocol id: {:?}", protocol_id,);

    let socket = UdpSocket::bind(local_addr).unwrap();
    socket
        .set_nonblocking(true)
        .expect("Can't set non-blocking mode");

    let connection_config = renet_connection_config();
    let server_config = ServerConfig::new(10, protocol_id, server_addr, *PRIVATE_KEY);
    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    RenetServer::new(current_time, server_config, connection_config, socket).unwrap()
}

#[derive(Deref, DerefMut, Debug, Clone)]
pub struct NetworkGameTimer(pub Timer);

impl Default for NetworkGameTimer {
    fn default() -> Self {
        Self(Timer::new(Duration::from_micros(15625 * 4), true))
    }
}

pub fn tick_network(time: Res<Time>, mut network_timer: ResMut<NetworkGameTimer>) {
    network_timer.tick(time.delta());
}

pub fn on_networktick(network_timer: Res<NetworkGameTimer>) -> bool {
    network_timer.just_finished()
}

pub struct SabiServerPlugin;

impl Plugin for SabiServerPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<ServerEntity>();

        app.insert_resource(Lobby::default());
        app.insert_resource(Reliable::<EntityUpdate>(EntityUpdate::new()));
        app.insert_resource(Unreliable::<EntityUpdate>(EntityUpdate::new()));
        app.insert_resource(new_renet_server());

        app.add_plugin(RenetServerPlugin);

        app.insert_resource(NetworkGameTimer::default());
        app.add_system(tick_network);

        app.add_system(
            server_send_interest_reliable
                .run_if(on_networktick)
                .label("send_interests"),
        );
        app.add_system_set(
            ConditionSet::new()
                .run_if(on_networktick)
                .before("send_interests")
                .with_system(server_queue_interest_reliable::<Transform>)
                .with_system(server_queue_interest_reliable::<GlobalTransform>)
                .with_system(server_queue_interest_reliable::<Velocity>)
                .with_system(server_queue_interest_reliable::<RigidBody>)
                .with_system(server_queue_interest_reliable::<Name>)
                .into(),
        );

        app.add_system(
            server_clear_reliable_queue
                .run_if(on_networktick)
                .after("send_interests"),
        );
        app.add_system(log_on_error_system);
    }
}

pub struct SabiClientPlugin;

impl Plugin for SabiClientPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<ServerEntity>();

        app.add_event::<(ServerEntity, ComponentsUpdate)>();

        app.insert_resource(Lobby::default());
        app.insert_resource(ServerEntities::default());
        app.insert_resource(new_renet_client());

        app.add_plugin(RenetClientPlugin);
        app.add_system(client_recv_interest_reliable.with_run_criteria(run_if_client_conected));
        app.add_system(client_update_reliable::<Transform>);
        app.add_system(client_update_reliable::<GlobalTransform>);
        app.add_system(client_update_reliable::<Velocity>);
        app.add_system(client_update_reliable::<RigidBody>);
        app.add_system(client_update_reliable::<Name>);

        app.add_system(log_on_error_system);
    }
}

fn log_on_error_system(mut renet_error: EventReader<RenetError>) {
    for renet_error in renet_error.iter() {
        error!("renet: {:?}", renet_error);
    }
}
