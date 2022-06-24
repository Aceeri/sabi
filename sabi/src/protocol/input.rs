use bevy::{
    prelude::*,
    utils::{Entry, HashMap},
};
use bevy_renet::renet::{RenetClient, RenetServer};

use serde::{Deserialize, Serialize};

use crate::prelude::*;

use super::{tick::NetworkAck, ClientId, NetworkTick};

pub const INPUT_RETAIN_BUFFER: i64 = 32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInputMessage<I>
where
    I: 'static + Send + Sync + Component + Clone + Default + Serialize,
{
    pub tick: NetworkTick,
    pub ack: NetworkAck,
    pub inputs: QueuedInputs<I>,
}

#[derive(Debug, Clone)]
pub struct PerClientQueuedInputs<I>
where
    I: 'static + Send + Sync + Component + Clone + Default + Serialize,
{
    clients: HashMap<ClientId, QueuedInputs<I>>,
}

impl<I> PerClientQueuedInputs<I>
where
    I: 'static + Send + Sync + Component + Clone + Default + Serialize,
{
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
        }
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
pub struct QueuedInputs<I>
where
    I: 'static + Send + Sync + Component + Clone + Default + Serialize,
{
    queue: HashMap<NetworkTick, I>,
}

impl<I> QueuedInputs<I>
where
    I: 'static + Send + Sync + Component + Clone + Default + Serialize,
{
    pub fn new() -> Self {
        Self {
            queue: HashMap::new(),
        }
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
        self.queue.insert(tick, input);
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

pub fn client_send_input<I>(
    tick: Res<NetworkTick>,
    input_buffer: Res<QueuedInputs<I>>,
    mut client: ResMut<RenetClient>,
) where
    I: 'static + Send + Sync + Component + Clone + Default + Serialize + for<'de> Deserialize<'de>,
{
    let message = ClientInputMessage {
        tick: tick.clone(),
        ack: NetworkAck::new(tick.clone()),
        inputs: input_buffer.clone(),
    };

    let input_message = bincode::serialize(&message).unwrap();
    let compressed_message = zstd::bulk::compress(&input_message.as_slice(), 0).unwrap();

    client.send_message(channel::CLIENT_INPUT, compressed_message);
}

pub fn client_update_input_buffer<I>(
    tick: Res<NetworkTick>,
    player_input: Res<I>,
    mut input_buffer: ResMut<QueuedInputs<I>>,
) where
    I: 'static + Send + Sync + Component + Clone + Default + Serialize + for<'de> Deserialize<'de>,
{
    input_buffer.push(*tick, player_input.clone());
}
