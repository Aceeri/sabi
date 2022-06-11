use bevy::prelude::*;
use bevy_renet::renet::{RenetServer, ServerConfig};

use std::{
    net::{Ipv4Addr, SocketAddrV4, ToSocketAddrs, UdpSocket},
    time::Duration,
};

use std::time::SystemTime;

use crate::protocol::*;

pub fn new_renet_server() -> RenetServer {
    let local_ip =
        crate::protocol::public_ip().unwrap_or(crate::protocol::localhost_ip().to_owned());
    let port = crate::protocol::PORT;

    let mut public_ip = None;
    // Set up ports using UPnP so people don't have to port forward.
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
                Ok(()) => {
                    info!("Forwarded port {} to {}", port, local_addr);
                }
                Err(ref err) => {
                    error!("failed to add port to gateway: {}", err);
                }
            }

            match gateway.get_external_ip() {
                Ok(external_ip) => {
                    public_ip = Some(external_ip);
                }
                Err(ref err) => {
                    error!("get_external_ip: {}", err);
                }
            }
        }
    };

    let external_ip = match public_ip {
        Some(ip) => ip.to_string(),
        None => my_internet_ip::get()
            .expect("failed to get external ip, cannot start server")
            .to_string(),
    };

    let server_addr = format!("{}:{}", external_ip, crate::protocol::PORT)
        .to_socket_addrs()
        .unwrap()
        .next()
        .unwrap();

    let local_addr = format!("{}:{}", local_ip, crate::protocol::PORT)
        .to_socket_addrs()
        .unwrap()
        .next()
        .unwrap();

    println!("binding to {:?}", server_addr);
    let protocol_id = crate::protocol::protocol_id();
    println!("protocol id: {:?}", protocol_id,);

    let socket = UdpSocket::bind(local_addr).unwrap();
    socket
        .set_nonblocking(true)
        .expect("Can't set non-blocking mode");

    let connection_config = crate::protocol::renet_connection_config();
    let server_config = ServerConfig::new(10, protocol_id, server_addr, *PRIVATE_KEY);
    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    RenetServer::new(current_time, server_config, connection_config, socket).unwrap()
}

pub fn server_clear_queue(mut updates: ResMut<EntityUpdate>) {
    updates.clear();
}

/// Priority accumulator across entities and components.
pub struct PriorityAccumulator {
    values: Vec<(f32, Entity, ReplicateId)>,
    unused: Vec<usize>,
    needs_sort: bool,

    sorted: Vec<(f32, Entity, ReplicateId)>,
    entity_map: HashMap<(Entity, ReplicateId), usize>,
}

impl PriorityAccumulator {
    pub fn new() -> Self {
        Self {
            values: Vec::new(),
            unused: Vec::new(),
            needs_sort: false,

            sorted: Vec::new(),
            entity_map: HashMap::new(),
        }
    }

    pub fn needs_sort(&mut self) {
        self.needs_sort = true;
    }

    pub fn get_or_insert_index(&mut self, entity: Entity, replicate_id: ReplicateId) -> usize {
        match self.entity_map.entry((entity, replicate_id)) {
            Entry::Occupied(occupied) => *occupied.get(),
            Entry::Vacant(vacant) => self.new_index(entity, replicate_id),
        }
    }

    pub fn new_index(&mut self, entity: Entity, replicate_id: ReplicateId) -> usize {
        if let Some(unused) = self.unused.pop() {
            self.entity_map.insert((entity, replicate_id), unused);
            unused
        } else {
            self.needs_sort();
            self.values.push((0.0, entity, replicate_id));
            let index = self.values.len() - 1;
            self.entity_map.insert((entity, replicate_id), index);
            index
        }
    }

    pub fn clear(&mut self, entity: Entity, replicate_id: ReplicateId) {
        self.needs_sort();
        let index = self.get_or_insert_index(entity, replicate_id);
        self.values[index] = (0.0, entity, replicate_id);
    }

    pub fn bump(&mut self, entity: Entity, replicate_id: ReplicateId, priority: f32) {
        self.needs_sort();
        let index = self.get_or_insert_index(entity, replicate_id);
        self.values[index].0 += priority;
    }

    pub fn clean(&mut self, entities: &Entities) {
        self.needs_sort();

        let mut mark = Vec::new();
        for (entity, replicate_id) in self.entity_map.keys() {
            if !entities.contains(*entity) {
                mark.push((*entity, *replicate_id));
            }
        }

        for index in mark {
            let index = self
                .entity_map
                .remove(&index)
                .expect("marked entity was not in map");

            self.values[index].0 = 0.0;
            self.unused.push(index);
        }
    }

    pub fn update_sorted(&mut self) {
        self.sorted = self.values.clone();
        self.sorted
            .sort_by(|(a, _, _), (b, _, _)| b.partial_cmp(a).unwrap());
        self.needs_sort = false;
    }

    pub fn sorted(&mut self) -> &Vec<(f32, Entity, ReplicateId)> {
        if self.needs_sort {
            self.update_sorted()
        }

        &self.sorted
    }
}

#[derive(Deref, DerefMut, Debug, Clone)]
pub struct ComponentsToSend(Vec<(Entity, ReplicateId)>);

impl ComponentsToSend {
    pub fn new() -> Self {
        Self(Vec::new())
    }
}

#[derive(Debug, Clone)]
pub struct ReplicateSizeEstimates(HashMap<ReplicateId, usize>);

impl ReplicateSizeEstimates {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn add(&mut self, id: ReplicateId, estimate: usize) {
        self.0.insert(id, estimate);
    }

    pub fn get(&self, id: &ReplicateId) -> usize {
        self.0.get(id).cloned().unwrap_or(0)
    }
}

#[derive(Deref, Debug, Clone)]
pub struct ReplicateMaxSize(usize);

impl Default for ReplicateMaxSize {
    fn default() -> Self {
        Self(100)
    }
}

pub fn fetch_top_priority(
    mut priority: ResMut<PriorityAccumulator>,
    estimate: Res<ReplicateSizeEstimates>,
    max: Res<ReplicateMaxSize>,
    mut to_send: ResMut<ComponentsToSend>,
) {
    let sorted = priority.sorted();

    to_send.clear();
    let mut used = 0usize;

    for (priority, entity, replicate_id) in sorted {
        let estimate = estimate.get(replicate_id);

        if used + estimate > max.0 {
            // We have used up our conservative estimated amount of bandwidth we can send
            break;
        }

        to_send.push((*entity, *replicate_id));
        used += estimate;
    }
}

pub fn server_bump_all<C>(
    mut priority: ResMut<PriorityAccumulator>,
    interest: Query<Entity, With<C>>,
) where
    C: 'static + Send + Sync + Component + Replicate + Clone,
{
    for entity in interest.iter() {
        priority.bump(entity, C::replicate_id(), 0.01);
    }
}

pub fn server_bump_changed<C>(
    mut priority: ResMut<PriorityAccumulator>,
    interest: Query<Entity, Changed<C>>,
) where
    C: 'static + Send + Sync + Component + Replicate + Clone,
{
    for entity in interest.iter() {
        priority.bump(entity, C::replicate_id(), 0.1);
    }
}

pub fn server_queue_interest<C>(
    mut priority: ResMut<PriorityAccumulator>,
    mut estimate: ResMut<ReplicateSizeEstimates>,
    mut updates: ResMut<EntityUpdate>,
    to_send: Res<ComponentsToSend>,
    query: Query<&C>,
) where
    C: 'static + Send + Sync + Component + Replicate + Clone,
{
    for (entity, replicate_id) in to_send.iter() {
        if *replicate_id == C::replicate_id() {
            if let Ok(component) = query.get(*entity) {
                let server_entity = ServerEntity::from_entity(*entity);
                let component_def = component.clone().into_def();
                let component_data = bincode::serialize(&component_def).unwrap();

                estimate.add(C::replicate_id(), component_data.len());

                let update = updates
                    .entry(server_entity)
                    .or_insert(ComponentsUpdate::new());
                update.insert(C::replicate_id(), component_data);

                //info!("clearing: {:?}, {:?}", entity, C::replicate_id());
                priority.clear(*entity, C::replicate_id());
            }
        }
    }
}

#[derive(Debug, Clone, Deref, DerefMut)]
pub struct BandwidthTimer(Timer);

impl BandwidthTimer {
    pub fn new() -> Self {
        Self(Timer::new(Duration::from_secs(1), true))
    }
}

pub fn display_server_bandwidth(
    time: Res<Time>,
    lobby: Res<Lobby>,
    mut timer: ResMut<BandwidthTimer>,
    mut server: ResMut<RenetServer>,
) {
    timer.tick(time.delta());

    if timer.just_finished() {
        for client_id in lobby.players.keys() {
            if let Some(network_info) = server.network_info(*client_id) {
                info!(
                    "client: {}, rtt: {:.0}, loss: {:.2}, skbps: {:.1}, rkbps: {:.1}",
                    client_id,
                    network_info.rtt,
                    network_info.packet_loss,
                    network_info.sent_bandwidth_kbps,
                    network_info.received_bandwidth_kbps,
                );
            }
        }
    }
}

pub fn server_send_interest(updates: Res<EntityUpdate>, mut server: ResMut<RenetServer>) {
    let data = bincode::serialize(&*updates).unwrap();
    let data = zstd::bulk::compress(&data.as_slice(), 0).unwrap();

    server.broadcast_message(COMPONENT, data);
}
