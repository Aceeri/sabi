use std::{marker::PhantomData, time::Duration};

use bevy::prelude::*;
use bevy_renet::{
    renet::{RenetClient, RenetError, RenetServer},
    run_if_client_conected, RenetClientPlugin,
};
use iyes_loopless::prelude::{ConditionHelpers, IntoConditionalSystem};
use serde::{Deserialize, Serialize};

use crate::{
    protocol::update::{server_send_interest, EntityUpdate},
    replicate::physics::ReplicatePhysicsPlugin,
    Replicate,
};

use crate::prelude::*;
use crate::protocol::*;

pub struct ReplicatePlugin<C>(PhantomData<C>)
where
    C: 'static + Send + Sync + Component + Replicate + Clone;

impl<C> Default for ReplicatePlugin<C>
where
    C: 'static + Send + Sync + Component + Replicate + Clone,
{
    fn default() -> Self {
        Self(PhantomData)
    }
}

#[derive(SystemLabel, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ServerQueueInterest;

impl<C> Plugin for ReplicatePlugin<C>
where
    C: 'static + Send + Sync + Component + Replicate + Clone,
{
    fn build(&self, app: &mut App) {
        app.add_system(
            crate::protocol::update::server_queue_interest::<C>
                .run_if(crate::protocol::on_network_tick)
                .run_if_resource_exists::<RenetServer>()
                .before("send_interests")
                .after("fetch_priority"),
        );

        app.add_system(
            crate::protocol::priority::server_bump_filtered::<C, With<C>, 1>
                .run_if(crate::protocol::on_network_tick)
                .run_if_resource_exists::<RenetServer>()
                .before("fetch_priority"),
        );

        app.add_system(
            crate::protocol::priority::server_bump_filtered::<C, Changed<C>, 100>
                .run_if(crate::protocol::on_network_tick)
                .run_if_resource_exists::<RenetServer>()
                .before("fetch_priority"),
        );

        app.add_system(
            crate::protocol::update::client_update::<C>
                .run_if_resource_exists::<RenetClient>()
                .run_if(client_connected)
                .after("client_recv_interest"),
        );
    }
}

#[derive(Debug, Clone)]
pub struct SabiPlugin<I> {
    pub phantom: PhantomData<I>, 
    pub tick_rate: Duration,
}

impl<I> Default for SabiPlugin<I> {
    fn default() -> Self {
        Self {
            phantom: PhantomData,
            tick_rate: tick_hz(32),
        }
    }
}

impl<I> Plugin for SabiPlugin<I>
where
    I: 'static + Send + Sync + Component + Clone + Default + Serialize + for<'de> Deserialize<'de>,
{
    fn build(&self, app: &mut App) {
        app.register_type::<ServerEntity>();

        app.add_event::<(ServerEntity, ComponentsUpdate)>();

        app.insert_resource(ServerEntities::default());
        app.insert_resource(EntityUpdate::new());
        app.insert_resource(NetworkTick::default());

        app.insert_resource(Lobby::default());
        app.insert_resource(NetworkGameTimer::new(self.tick_rate));

        app.add_plugin(ReplicatePhysicsPlugin);
        app.add_plugin(SabiServerPlugin::<I>::default());
        app.add_plugin(SabiClientPlugin::<I>::default());

        app.add_system(tick_network);

        app.add_plugin(ReplicatePlugin::<Transform>::default());
        app.add_plugin(ReplicatePlugin::<GlobalTransform>::default());
        app.add_plugin(ReplicatePlugin::<Name>::default());

        app.insert_resource(PreviousRenetError(None));
        app.add_system(log_on_error_system);
    }
}

#[derive(Debug, Clone)]
pub struct SabiServerPlugin<I>(PhantomData<I>);

impl<I> Default for SabiServerPlugin<I> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<I> Plugin for SabiServerPlugin<I>
where
    I: 'static + Send + Sync + Component + Clone + Default + Serialize + for<'de> Deserialize<'de>,
{
    fn build(&self, app: &mut App) {
        app.insert_resource(crate::protocol::priority::PriorityAccumulator::new());
        app.insert_resource(crate::protocol::priority::ReplicateSizeEstimates::new());
        app.insert_resource(crate::protocol::priority::ReplicateMaxSize::default());
        app.insert_resource(crate::protocol::priority::ComponentsToSend::new());
        app.insert_resource(crate::protocol::input::PerClientQueuedInputs::<I>::new());

        app.add_plugin(bevy_renet::RenetServerPlugin);
        app.add_plugin(bevy_renet::RenetClientPlugin);

        app.add_system(
            crate::protocol::input::server_recv_input::<I>
                .run_if_resource_exists::<RenetServer>()
                .run_if(on_network_tick)
                .label("recv_input"),
        );

        app.add_system(
            crate::protocol::input::server_apply_input::<I>
                .run_if_resource_exists::<RenetServer>()
                .run_if(on_network_tick)
                .label("apply_input")
                .after("recv_input"),
        );

        app.add_system(
            crate::protocol::priority::fetch_top_priority
                .run_if_resource_exists::<RenetServer>()
                .run_if(on_network_tick)
                .label("fetch_priority"),
        );
        app.add_system(
            server_send_interest
                .run_if_resource_exists::<RenetServer>()
                .run_if(on_network_tick)
                .label("send_interests"),
        );

        app.add_system(
            crate::protocol::update::server_clear_queue
                .run_if(on_network_tick)
                .after("send_interests"),
        );
        app.add_system(log_on_error_system);
    }
}

#[derive(Debug, Clone)]
pub struct SabiClientPlugin<I>(PhantomData<I>);

impl<I> Default for SabiClientPlugin<I> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<I> Plugin for SabiClientPlugin<I>
where
    I: 'static + Send + Sync + Component + Clone + Default + Serialize + for<'de> Deserialize<'de>,
{
    fn build(&self, app: &mut App) {
        app.add_plugin(RenetClientPlugin);
        app.insert_resource(crate::protocol::update::UpdateMessages::new());

        app.add_system(
            crate::protocol::update::client_recv_interest
                .run_if_resource_exists::<RenetClient>()
                .run_if(client_connected)
                .label("client_recv_interest"),
        );

        app.add_system(
            crate::protocol::input::client_update_input_buffer::<I>
                .run_if_resource_exists::<RenetClient>()
                .run_if(client_connected)
                .run_if(on_network_tick)
                .label("client_update_input_buffer")
                .before("client_send_input"),
        );

        app.add_system(
            crate::protocol::input::client_send_input::<I>
                .run_if_resource_exists::<RenetClient>()
                .run_if(client_connected)
                .run_if(on_network_tick)
                .label("client_send_input")
                .after("client_update_input_buffer"),
        );
    }
}

#[derive(Debug)]
pub struct PreviousRenetError(Option<String>);

pub fn log_on_error_system(
    mut previous: ResMut<PreviousRenetError>,
    mut renet_error: EventReader<RenetError>,
) {
    for err in renet_error.iter() {
        if let Some(previous_err) = &previous.0 {
            if previous_err == &err.to_string() {
                continue;
            }
        }

        error!("{}", err);
        previous.0 = Some(err.to_string());
    }
}
