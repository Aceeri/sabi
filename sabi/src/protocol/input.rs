use bevy::prelude::*;
use bevy_renet::renet::{RenetClient, RenetServer};

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::prelude::*;

use super::NetworkTick;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInputBuffer<I>
where
    I: 'static + Send + Sync + Component + Clone + Default + Serialize,
    // + for<'de> Deserialize<'de>, can't do this because of Deserialize derive
{
    buffer: VecDeque<I>,
}

impl<I> ClientInputBuffer<I>
where
    I: 'static + Send + Sync + Component + Clone + Default + Serialize + for<'de> Deserialize<'de>,
{
    pub fn new(len: usize) -> Self {
        Self {
            buffer: (0..len).map(|_| I::default()).collect::<VecDeque<I>>(),
        }
    }

    pub fn push(&mut self, input: I) {
        self.buffer.push_back(input);
        self.buffer.pop_front();
    }

    pub fn view(&self) -> impl Iterator<Item = &I> {
        self.buffer.iter()
    }

    pub fn back(&self) -> Option<&I> {
        self.buffer.back()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInputMessage<I>
where
    I: 'static + Send + Sync + Component + Clone + Default + Serialize,
{
    pub tick: NetworkTick,
    pub buffer: ClientInputBuffer<I>,
}

pub fn server_recv_input<I>(
    mut commands: Commands,
    mut server: ResMut<RenetServer>,
    lobby: Res<Lobby>,
) where
    I: 'static + Send + Sync + Component + Clone + Default + Serialize + for<'de> Deserialize<'de>,
{
    for client_id in server.clients_id().into_iter() {
        while let Some(message) = server.receive_message(client_id, channel::CLIENT_INPUT) {
            let decompressed = zstd::bulk::decompress(&message.as_slice(), 10 * 1024).unwrap();
            let input_message: ClientInputMessage<I> = bincode::deserialize(&decompressed).unwrap();

            if let Some(player_entity) = lobby.players.get(&client_id) {
                if let Some(input) = input_message.buffer.back() {
                    commands.entity(*player_entity).insert(input.clone());
                }
            }
        }
    }
}

pub fn client_send_input<I>(
    tick: Res<NetworkTick>,
    input_buffer: Res<ClientInputBuffer<I>>,
    mut client: ResMut<RenetClient>,
) where
    I: 'static + Send + Sync + Component + Clone + Default + Serialize + for<'de> Deserialize<'de>,
{
    let message = ClientInputMessage {
        tick: tick.clone(),
        buffer: input_buffer.clone(),
    };

    let input_message = bincode::serialize(&message).unwrap();
    let compressed_message = zstd::bulk::compress(&input_message.as_slice(), 0).unwrap();

    client.send_message(channel::CLIENT_INPUT, compressed_message);
}

pub fn client_update_input_buffer<I>(
    player_input: Res<I>,
    mut input_buffer: ResMut<ClientInputBuffer<I>>,
) where
    I: 'static + Send + Sync + Component + Clone + Default + Serialize + for<'de> Deserialize<'de>,
{
    input_buffer.push(player_input.clone());
}
