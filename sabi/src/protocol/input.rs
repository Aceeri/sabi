use std::collections::VecDeque;
use std::{collections::BTreeMap, time::Duration};
use std::fmt::Debug;

use bevy::{
    prelude::*,
    utils::{Entry, HashMap},
};

use bevy::ecs::entity::Entities;
use bevy_renet::renet::{RenetClient, RenetServer};

use serde::{Deserialize, Serialize};

use crate::prelude::*;

use super::{
    ack::{ClientAcks, NetworkAck},
    ClientId, NetworkTick,
};

/// How many inputs we should retain for replaying inputs.
pub const INPUT_RETAIN_BUFFER: i64 = 32;
/// How many inputs we should send to the server for future ticks.
/// 
/// TODO: These should probably be determined by RTT and time dilation.
/// We probably should send less than the frame buffer since by the time it
/// gets to the server, most of these inputs will be late.
pub const INPUT_SEND_BUFFER: i64 = 6;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputDeviation {
    pub mean: f32,
    pub deviation: f32, 
}

#[derive(Default, Debug, Clone)]
pub struct ClientReceivedHistory {
    clients: BTreeMap<ClientId, ReceivedHistory>
}

impl ClientReceivedHistory {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn push(&mut self, client_id: ClientId, sample: Duration) {
        self.clients.entry(client_id).or_default().push(sample);
    }

    pub fn deviation(&mut self, client_id: ClientId) -> InputDeviation {
        self.clients.entry(client_id).or_default().deviation()
    }
}

#[derive(Default, Debug, Clone)]
pub struct ReceivedHistory {
    previous: Option<Duration>,
    times: VecDeque<f32>,
}

impl ReceivedHistory {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn push(&mut self, sample: Duration) {
        if let Some(previous) = self.previous {
            let new_sample = previous.saturating_sub(sample);
            self.times.push_back(new_sample.as_secs_f32());

            if self.times.len() > 64 {
                self.times.pop_front();
            }
        }
        
        self.previous = Some(sample);
    }

    pub fn deviation(&self) -> InputDeviation {
        let samples = self.times.len() as f32;
        let sum: f32 = self.times.iter().sum();
        let mean = sum / samples;

        let deviations_sum: f32 = self.times.iter().map(|sample| (sample - mean) * (sample - mean)).sum();
        let variance = deviations_sum / samples;
        let standard_deviation = variance.sqrt();

        InputDeviation {
            mean: mean,
            deviation: standard_deviation,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInputMessage<I> {
    pub tick: NetworkTick,
    pub ack: NetworkAck,
    pub inputs: QueuedInputs<I>,
}

#[derive(Debug, Clone)]
pub struct ClientQueuedInputs<I> {
    clients: HashMap<ClientId, QueuedInputs<I>>,
}

impl<I> ClientQueuedInputs<I> {
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
        self.retain(INPUT_RETAIN_BUFFER);
    }

    /// Retain any in the queue that are within a buffer range.
    pub fn retain(&mut self, buffer: i64) {
        let newest = self.queue.keys().max().cloned().unwrap_or_default();

        self.queue
            .retain(|tick, _| (newest.tick() as i64) - (tick.tick() as i64) < buffer);
    }
}

pub fn server_recv_input<I>(
    time: Res<Time>,
    mut recv_history: ResMut<ClientReceivedHistory>,
    tick: Res<NetworkTick>,
    mut server: ResMut<RenetServer>,
    mut queued_inputs: ResMut<ClientQueuedInputs<I>>,
    mut acks: ResMut<ClientAcks>,
) where
    I: 'static + Send + Sync + Component + Clone + Default + Serialize + for<'de> Deserialize<'de>,
{
    queued_inputs.clean_old(*tick);

    for client_id in server.clients_id().into_iter() {
        while let Some(message) = server.receive_message(client_id, channel::CLIENT_INPUT) {
            let decompressed = zstd::bulk::decompress(&message.as_slice(), 10 * 1024).unwrap();
            let input_message: ClientInputMessage<I> = bincode::deserialize(&decompressed).unwrap();

            recv_history.push(client_id, time.time_since_startup());
            acks.apply_ack(client_id, &input_message.ack);
            queued_inputs.upsert(client_id, input_message.inputs);
        }
    }
}

pub fn server_apply_input<I>(
    mut commands: Commands,
    entities: &Entities,
    tick: Res<NetworkTick>,
    queued_inputs: Res<ClientQueuedInputs<I>>,
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
            //error!("no input for player {} on tick {}", client, tick.tick());
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
    let mut send_buffer = input_buffer.clone();
    send_buffer.retain(INPUT_SEND_BUFFER);

    let message = ClientInputMessage {
        tick: tick.clone(),
        ack: NetworkAck::new(tick.clone()),
        inputs: send_buffer,
    };

    let serialized = bincode::serialize(&message).unwrap();
    //crate::message_sample::try_add_sample("input", &serialized);
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
