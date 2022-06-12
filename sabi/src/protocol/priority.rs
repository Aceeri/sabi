use bevy::prelude::*;
use bevy_renet::renet::{RenetServer, ServerConfig};

use std::{
    net::{Ipv4Addr, SocketAddrV4, ToSocketAddrs, UdpSocket},
    time::Duration,
};

use std::time::SystemTime;

use crate::protocol::*;

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
