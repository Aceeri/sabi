use std::collections::BTreeMap;
use std::fmt::Debug;

use bevy::{
    prelude::*,
    utils::{Entry, HashMap},
};

use bevy::ecs::entity::Entities;
use bevy_renet::renet::{RenetClient, RenetServer};

use serde::{Deserialize, Serialize};

use crate::prelude::*;

use super::{tick::NetworkAck, ClientId, NetworkTick};

pub const TARGET_PING: i64 = 60;
pub const INPUT_RETAIN_BUFFER: i64 = 32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInputMessage<I> {
    pub tick: NetworkTick,
    pub ack: NetworkAck,
    pub inputs: QueuedInputs<I>,
}

#[derive(Debug, Clone)]
pub struct PerClientQueuedInputs<I> {
    clients: HashMap<ClientId, QueuedInputs<I>>,
}

impl<I> PerClientQueuedInputs<I> {
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
        }
    }

    pub fn get(&self, client: ClientId, tick: &NetworkTick) -> Option<&I> {
        self.clients.get(&client).and_then(|queue| queue.get(tick))
    }

    pub fn upsert(&mut self, client: ClientId, input: QueuedInputs<I>) {
        match self.clients.entry(client) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().apply_buffer(input);
            }
            Entry::Vacant(entry) => {
                entry.insert(input);
            }
        }
    }

    pub fn clean_old(&mut self, current: NetworkTick) {
        for (_, queue) in &mut self.clients {
            queue.clean_old(current);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedInputs<I> {
    queue: BTreeMap<NetworkTick, I>,
}

impl<I> QueuedInputs<I> {
    pub fn new() -> Self {
        Self {
            queue: Default::default(),
        }
    }

    pub fn get(&self, tick: &NetworkTick) -> Option<&I> {
        self.queue.get(tick)
    }

    pub fn apply_buffer(&mut self, other: Self) {
        for (tick, input) in other.queue {
            self.upsert(tick, input);
        }
    }

    /// Upsert inputs, but reject inserting for previous ticks.
    pub fn upsert_reject(&mut self, current: NetworkTick, tick: NetworkTick, input: I) {
        if tick.tick() < current.tick() {
            return;
        }

        self.upsert(tick, input);
    }

    pub fn upsert(&mut self, tick: NetworkTick, input: I) {
        if let None = self.queue.insert(tick, input) {
            //info!("recv input for tick {}", tick.tick());
        }
    }

    /// Clean any in the queue that are before the current tick.
    pub fn clean_old(&mut self, current: NetworkTick) {
        self.queue.retain(|tick, _| current.tick() >= tick.tick());
    }

    /// Push an input into the queue
    pub fn push(&mut self, tick: NetworkTick, input: I) {
        self.queue.insert(tick, input);
        self.retain();
    }

    /// Retain any in the queue that are within a buffer range.
    pub fn retain(&mut self) {
        let newest = self.queue.keys().max().cloned().unwrap_or_default();

        self.queue
            .retain(|tick, _| (newest.tick() as i64) - (tick.tick() as i64) < INPUT_RETAIN_BUFFER);
    }
}

pub fn server_recv_input<I>(
    tick: Res<NetworkTick>,
    mut server: ResMut<RenetServer>,
    mut queued_inputs: ResMut<PerClientQueuedInputs<I>>,
) where
    I: 'static + Send + Sync + Component + Clone + Default + Serialize + for<'de> Deserialize<'de>,
{
    queued_inputs.clean_old(*tick);

    for client_id in server.clients_id().into_iter() {
        while let Some(message) = server.receive_message(client_id, channel::CLIENT_INPUT) {
            let decompressed = zstd::bulk::decompress(&message.as_slice(), 10 * 1024).unwrap();
            let input_message: ClientInputMessage<I> = bincode::deserialize(&decompressed).unwrap();

            queued_inputs.upsert(client_id, input_message.inputs);
        }
    }
}

pub fn server_apply_input<I>(
    mut commands: Commands,
    entities: &Entities,
    tick: Res<NetworkTick>,
    queued_inputs: Res<PerClientQueuedInputs<I>>,
    lobby: Res<Lobby>,
) where
    I: 'static + Send + Sync + Component + Clone + Default + Serialize + for<'de> Deserialize<'de>,
{
    for (client, entity) in lobby.players.iter() {
        if let Some(input) = queued_inputs.get(*client, &tick) {
            if entities.contains(*entity) {
                commands.entity(*entity).insert(input.clone());
            }
        } else {
            error!("no input for player {} on tick {}", client, tick.tick());
        }
    }
}

pub fn client_send_input<I>(
    tick: Res<NetworkTick>,
    input_buffer: Res<QueuedInputs<I>>,
    mut client: ResMut<RenetClient>,
) where
    I: 'static
        + Send
        + Sync
        + Component
        + Clone
        + Default
        + Serialize
        + for<'de> Deserialize<'de>
        + std::fmt::Debug,
{
    let message = ClientInputMessage {
        tick: tick.clone(),
        ack: NetworkAck::new(tick.clone()),
        inputs: input_buffer.clone(),
    };

    let serialized = bincode::serialize(&message).unwrap();
    crate::message_sample::try_add_sample("input", &serialized);
    let compressed = zstd::bulk::compress(&serialized.as_slice(), 0).unwrap();

    client.send_message(channel::CLIENT_INPUT, compressed);
}

pub fn client_update_input_buffer<I>(
    tick: Res<NetworkTick>,
    player_input: Res<I>,
    mut input_buffer: ResMut<QueuedInputs<I>>,
) where
    I: 'static
        + Send
        + Sync
        + Component
        + Clone
        + Default
        + Serialize
        + for<'de> Deserialize<'de>
        + Debug,
{
    input_buffer.push(*tick, player_input.clone());
}

pub fn client_apply_input_buffer<I>(
    tick: Res<NetworkTick>,
    mut player_input: ResMut<I>,
    input_buffer: Res<QueuedInputs<I>>,
) where
    I: 'static
        + Send
        + Sync
        + Component
        + Clone
        + Default
        + Serialize
        + for<'de> Deserialize<'de>
        + Debug,
{
    if let Some(input) = input_buffer.get(&*tick) {
        //info!("input: {:?}", input);
        *player_input = input.clone();
    } else {
        error!("no input for tick {}", tick.tick());
    }
}
