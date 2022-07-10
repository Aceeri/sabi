use std::{marker::PhantomData, time::Duration};

use bevy::prelude::*;
use bevy_renet::{
    renet::{RenetClient, RenetError, RenetServer},
    RenetClientPlugin,
};
use iyes_loopless::prelude::{ConditionHelpers, IntoConditionalSystem};
use serde::{Deserialize, Serialize};

use crate::{
    protocol::{
        resim::SnapshotBuffer,
        update::{server_send_interest, EntityUpdate},
    },
    replicate::physics::ReplicatePhysicsPlugin,
    stage::{
        NetworkCoreStage, NetworkSimulationAppExt, NetworkSimulationInfo, NetworkSimulationStage,
        NetworkStage,
    },
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
        if app.world.contains_resource::<crate::Server>() {
            app.add_meta_network_system(
                crate::protocol::update::server_queue_interest::<C>
                    //.run_if_resource_exists::<RenetServer>()
                    .before("send_interests")
                    .after("fetch_priority"),
            );

            app.add_meta_network_system(
                crate::protocol::priority::server_bump_filtered::<C, With<C>, 1>
                    //.run_if_resource_exists::<RenetServer>()
                    .before("fetch_priority"),
            );

            app.add_meta_network_system(
                crate::protocol::priority::server_bump_filtered::<C, Changed<C>, 100>
                    //.run_if_resource_exists::<RenetServer>()
                    .before("fetch_priority"),
            );
        }

        if app.world.contains_resource::<crate::Client>() {
            app.insert_resource(SnapshotBuffer::<C>::new());
            app.add_apply_update_network_system(
                crate::protocol::update::client_update::<C>.after("client_apply_server_update"),
            );

            app.add_meta_network_system(
                crate::protocol::resim::store_snapshot::<C>
                    .run_if_resource_exists::<RenetClient>()
                    .run_if(client_connected),
            );
            app.add_rewind_network_system(crate::protocol::resim::rewind::<C>);
        }
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
    fn build(&self, app: &mut App) {
        app.register_type::<ServerEntity>();

        app.add_event::<(ServerEntity, ComponentsUpdate)>();
        app.add_stage_before(
            CoreStage::Update,
            NetworkStage,
            NetworkSimulationStage::new(self.tick_rate),
        );

        app.add_network_stage(NetworkCoreStage::Update, SystemStage::parallel());
        app.add_network_stage_before(
            NetworkCoreStage::Update,
            NetworkCoreStage::PreUpdate,
            SystemStage::parallel(),
        );
        app.add_network_stage_before(
            NetworkCoreStage::PreUpdate,
            NetworkCoreStage::First,
            SystemStage::parallel(),
        );
        app.add_network_stage_after(
            NetworkCoreStage::Update,
            NetworkCoreStage::PostUpdate,
            SystemStage::parallel(),
        );
        app.add_network_stage_after(
            NetworkCoreStage::PostUpdate,
            NetworkCoreStage::Last,
            SystemStage::parallel(),
        );

        app.insert_resource(ServerEntities::default());
        app.insert_resource(EntityUpdate::new());
        app.insert_resource(NetworkTick::default());
        app.insert_resource(NetworkSimulationInfo::new(self.tick_rate));

        app.insert_resource(Lobby::default());

        app.add_plugin(ReplicatePhysicsPlugin);
        if app.world.contains_resource::<crate::Server>() {
            app.add_plugin(SabiServerPlugin::<I>::default());
        }

        if app.world.contains_resource::<crate::Client>() {
            app.add_plugin(SabiClientPlugin::<I>::default());
        }

        app.add_system_to_network_stage(NetworkCoreStage::Last, increment_network_tick);

        //app.add_apply_update_network_system(bevy::transform::transform_propagate_system);

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

        app.add_meta_network_system(
            crate::protocol::input::server_recv_input::<I>
                .run_if_resource_exists::<RenetServer>()
                .label("recv_input"),
        );

        app.add_meta_network_system(
            crate::protocol::input::server_apply_input::<I>
                .run_if_resource_exists::<RenetServer>()
                .label("apply_input")
                .after("recv_input"),
        );

        app.add_meta_network_system(
            crate::protocol::priority::fetch_top_priority
                .run_if_resource_exists::<RenetServer>()
                .label("fetch_priority"),
        );
        app.add_meta_network_system(
            server_send_interest
                .run_if_resource_exists::<RenetServer>()
                .label("send_interests"),
        );

        app.add_meta_network_system(
            crate::protocol::update::server_clear_queue.after("send_interests"),
        );
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
    fn build(&self, app: &mut App) {
        app.add_plugin(RenetClientPlugin);
        app.insert_resource(crate::protocol::update::UpdateMessages::new());

        app.add_meta_network_system(
            crate::protocol::update::client_recv_interest
                .run_if_resource_exists::<RenetClient>()
                .run_if(client_connected)
                .label("client_recv_interest"),
        );

        app.add_apply_update_network_system(
            crate::protocol::update::client_apply_server_update
                //.run_if_resource_exists::<RenetClient>()
                //.run_if(client_connected)
                .label("client_apply_server_update"),
        );

        app.add_network_system(
            crate::protocol::input::client_update_input_buffer::<I>
                .run_if_resource_exists::<RenetClient>()
                .run_if(client_connected)
                .label("client_update_input_buffer")
                .before("client_send_input"),
        );

        app.add_meta_network_system(
            crate::protocol::input::client_send_input::<I>
                .run_if_resource_exists::<RenetClient>()
                .run_if(client_connected)
                .label("client_send_input")
                .before("client_recv_interest")
                .after("client_update_input_buffer"),
        );

        app.add_apply_update_network_system(
            crate::protocol::input::client_apply_input_buffer::<I>
                //.run_if(client_connected)
                .label("client_apply_input_buffer"),
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
