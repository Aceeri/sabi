use bevy::{
    ecs::entity::Entities,
    prelude::*,
    reflect::FromReflect,
    utils::{Entry, HashMap},
};
use bevy_renet::renet::{
    BlockChannelConfig, ChannelConfig, ReliableChannelConfig, RenetClient, RenetConnectionConfig,
    RenetServer, UnreliableChannelConfig, NETCODE_KEY_BYTES,
};

use std::{
    collections::VecDeque,
    hash::{Hash, Hasher},
    net::UdpSocket,
    time::Duration,
};

use serde::{Deserialize, Serialize};

use crate::prelude::*;

use super::NetworkTick;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInputBuffer<I>
where
    I: 'static + Send + Sync + Clone + Serialize,
{
    buffer: VecDeque<I>,
    #[serde(skip)]
    len: usize,
}

impl<I> ClientInputBuffer<I>
where
    I: 'static + Send + Sync + Clone + Serialize + for<'de> Deserialize<'de>,
{
    pub fn new(len: usize) -> Self {
        Self {
            buffer: VecDeque::new(),
            len: len,
        }
    }

    pub fn push(&mut self, input: I) {
        self.buffer.push_back(input);

        if self.buffer.len() > self.len {
            self.buffer.pop_front();
        }
    }

    pub fn view(&self) -> impl Iterator<Item = &I> {
        self.buffer.iter()
    }
}

pub fn server_recv_input<I>(
    mut commands: Commands,
    mut server: ResMut<RenetServer>,
    lobby: Res<Lobby>,
) where
    I: 'static + Send + Sync + Clone + Serialize + for<'de> Deserialize<'de>,
{
    for client_id in server.clients_id().into_iter() {
        while let Some(message) = server.receive_message(client_id, channel::CLIENT_INPUT) {
            let decompressed = zstd::bulk::decompress(&message.as_slice(), 10 * 1024).unwrap();
            let player_input: ClientInputBuffer<I> = bincode::deserialize(&decompressed).unwrap();

            if let Some(player_entity) = lobby.players.get(&client_id) {
                if let Some(input) = player_input.buffer.back() {
                    //commands.entity(*player_entity).insert(*input);
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
    I: 'static + Send + Sync + Clone + Serialize + for<'de> Deserialize<'de>,
{
    let input_message = bincode::serialize(&*input_buffer).unwrap();
    let compressed_message = zstd::bulk::compress(&input_message.as_slice(), 0).unwrap();

    client.send_message(channel::CLIENT_INPUT, compressed_message);
}

pub fn client_update_input_buffer<I>(
    player_input: Res<I>,
    mut input_buffer: ResMut<ClientInputBuffer<I>>,
) where
    I: 'static + Send + Sync + Clone + Serialize + for<'de> Deserialize<'de>,
{
    input_buffer.push(player_input.clone());
}
