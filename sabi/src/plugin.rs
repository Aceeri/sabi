use std::{marker::PhantomData, time::Duration};

use bevy::prelude::*;
#[cfg(feature = "public")]
use bevy_renet::{
    renet::{RenetClient, RenetError, RenetServer},
    RenetClientPlugin,
};
use iyes_loopless::prelude::{ConditionHelpers, IntoConditionalSystem};
use serde::{Deserialize, Serialize};

use crate::stage::{
    NetworkCoreStage, NetworkSimulationAppExt, NetworkSimulationInfo, NetworkSimulationStage,
    NetworkStage,
};
#[cfg(feature = "public")]
use crate::{
    protocol::{
        resim::SnapshotBuffer,
        update::{server_send_interest, EntityUpdate},
    },
    //replicate::physics2d::ReplicatePhysics2dPlugin,
    replicate::physics3d::ReplicatePhysics3dPlugin,
};

use crate::prelude::*;
#[cfg(feature = "public")]
use crate::protocol::*;

#[cfg(feature = "public")]
pub struct ReplicatePlugin<C>(PhantomData<C>)
where
    C: 'static + Component + Reflect + FromReflect + Clone;

#[cfg(feature = "public")]
impl<C> Default for ReplicatePlugin<C>
where
    C: 'static + Component + Reflect + FromReflect + Clone,
{
    fn default() -> Self {
        Self(PhantomData)
    }
}

#[derive(SystemLabel, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ServerQueueInterest;

#[cfg(feature = "public")]
impl<C> Plugin for ReplicatePlugin<C>
where
    C: 'static + Component + Reflect + FromReflect + Clone,
{
    fn build(&self, app: &mut App) {
        if app.world.contains_resource::<crate::Server>() {
            app.add_meta_network_system(
                crate::protocol::update::server_queue_interest::<C>
                    .before("server_send_interest")
                    .after("queue_interests"),
            );

            app.add_meta_network_system(crate::protocol::interest::component_changes::<C>);

            app.add_meta_network_system(
                crate::protocol::interest::baseload_components::<C>.before("clear_baseload"),
            );
        }

        if app.world.contains_resource::<crate::Client>() {
            app.insert_resource(SnapshotBuffer::<C>::new());
            app.add_update_history_network_system(
                crate::protocol::update::client_update::<C>.after("client_apply_server_update"),
            );

            app.add_meta_network_system(
                crate::protocol::resim::store_snapshot::<C>
                    .run_if_resource_exists::<RenetClient>()
                    .run_if_resource_exists::<NetworkTick>()
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
        + Resource
        + Component
        + Clone
        + Default
        + Serialize
        + for<'de> Deserialize<'de>
        + std::fmt::Debug
        + Resource,
{
    fn build(&self, app: &mut App) {
        app.world
            .init_resource::<crate::protocol::demands::ReplicateDemands>();

        if app.world.contains_resource::<crate::Local>() {
            info!("initiating as local");
            app.world.remove_resource::<crate::Server>();
            app.world.init_resource::<crate::Client>();
        }

        match (
            app.world.contains_resource::<crate::Client>(),
            app.world.contains_resource::<crate::Server>(),
        ) {
            (true, false) => info!("initiating as client"),
            (false, true) => info!("initiating as server"),
            (true, true) => panic!("initiating as client and server"),
            (false, false) => panic!("requires `sabi::Client` or `sabi::Server` to start"),
        }

        #[cfg(feature = "public")]
        app.register_type::<ServerEntity>();

        #[cfg(feature = "public")]
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

        #[cfg(feature = "public")]
        app.insert_resource(ServerEntities::default());
        #[cfg(feature = "public")]
        app.insert_resource(EntityUpdate::new());
        app.init_resource::<NetworkTick>();
        if !app.world.contains_resource::<NetworkSimulationInfo>() {
            app.insert_resource(NetworkSimulationInfo::new(self.tick_rate));
        }

        app.insert_resource(Lobby::default());

        #[cfg(feature = "public")]
        app.add_plugin(ReplicatePhysics3dPlugin);
        //app.add_plugin(ReplicatePhysics2dPlugin);
        if app.world.contains_resource::<crate::Server>() {
            #[cfg(feature = "public")]
            app.add_plugin(SabiServerPlugin::<I>::default());
        }

        if app.world.contains_resource::<crate::Client>() {
            #[cfg(feature = "public")]
            app.add_plugin(SabiClientPlugin::<I>::default());
        }

        //app.add_system_to_network_stage(NetworkCoreStage::Last, increment_network_tick);

        //app.add_apply_update_network_system(bevy::transform::transform_propagate_system);

        #[cfg(feature = "public")]
        app.add_plugin(ReplicatePlugin::<Transform>::default());
        #[cfg(feature = "public")]
        app.add_plugin(ReplicatePlugin::<GlobalTransform>::default());
        #[cfg(feature = "public")]
        app.add_plugin(ReplicatePlugin::<Name>::default());

        app.insert_resource(PreviousRenetError(None));
        #[cfg(feature = "public")]
        app.add_system(handle_renet_error);
        #[cfg(feature = "public")]
        app.add_system(handle_client_disconnect);
    }
}

#[derive(Debug, Clone)]
pub struct SabiServerPlugin<I>(PhantomData<I>);

impl<I> Default for SabiServerPlugin<I> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

#[cfg(feature = "public")]
impl<I> Plugin for SabiServerPlugin<I>
where
    I: 'static
        + Send
        + Sync
        + Resource
        + Component
        + Clone
        + Default
        + Serialize
        + for<'de> Deserialize<'de>,
{
    fn build(&self, app: &mut App) {
        app.insert_resource(crate::protocol::interest::InterestsToSend::new());
        app.insert_resource(crate::protocol::interest::ClientInterestQueues::new());
        app.insert_resource(crate::protocol::interest::Baseload::new());
        app.insert_resource(crate::protocol::interest::ClientUnackedInterests::new());
        //app.insert_resource(crate::protocol::interest::SentInterests::new());

        app.insert_resource(crate::protocol::update::ClientEntityUpdates::new());

        app.insert_resource(crate::protocol::ack::ClientAcks::new());

        app.insert_resource(crate::protocol::demands::ReplicateSizeEstimates::new());
        app.insert_resource(crate::protocol::demands::ReplicateMaxSize::default());
        app.insert_resource(crate::protocol::input::ClientQueuedInputs::<I>::new());
        app.insert_resource(crate::protocol::input::ClientReceivedHistory::new());

        app.add_plugin(bevy_renet::RenetServerPlugin {
            clear_events: false,
        });

        app.add_network_system_set(bevy_renet::RenetServerPlugin::get_clear_event_systems());

        app.add_system(crate::protocol::interest::setup_baseload.label("setup_baseload"));
        app.add_meta_network_system(
            crate::protocol::interest::clear_baseloads.label("clear_baseload"),
        );

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
            crate::protocol::interest::queue_interests.label("queue_interests"),
        );

        app.add_meta_network_system(
            server_send_interest
                .run_if_resource_exists::<RenetServer>()
                .label("server_send_interest"),
        );

        app.add_meta_network_system(
            crate::protocol::update::server_clear_queue.after("server_send_interest"),
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

#[cfg(feature = "public")]
impl<I> Plugin for SabiClientPlugin<I>
where
    I: 'static
        + Send
        + Sync
        + Resource
        + Component
        + Clone
        + Default
        + Serialize
        + for<'de> Deserialize<'de>
        + std::fmt::Debug
        + Resource,
{
    fn build(&self, app: &mut App) {
        app.add_plugin(RenetClientPlugin {
            clear_events: false,
        });
        app.add_network_system_set(RenetClientPlugin::get_clear_event_systems());

        app.insert_resource(crate::protocol::update::UpdateMessages::new());

        app.add_meta_network_system(
            crate::protocol::update::client_recv_interest
                .run_if_resource_exists::<RenetClient>()
                .run_if(client_connected)
                .label("client_recv_interest"),
        );
        app.add_update_history_network_system(
            crate::protocol::update::client_apply_server_update
                .run_if_resource_exists::<RenetClient>()
                .run_if_resource_exists::<NetworkTick>()
                .label("client_apply_server_update"),
        );

        app.add_meta_network_system(
            crate::protocol::input::client_update_input_buffer::<I>
                .run_if_resource_exists::<NetworkTick>()
                .label("client_update_input_buffer"),
        );
        app.add_meta_network_system(
            crate::protocol::input::client_send_input::<I>
                .run_if_resource_exists::<RenetClient>()
                .run_if_resource_exists::<NetworkTick>()
                .run_if(client_connected)
                .label("client_send_input")
                .before("client_recv_interest")
                .after("client_update_input_buffer"),
        );

        app.add_input_history_network_system(
            crate::protocol::input::client_apply_input_buffer::<I>
                .run_if_resource_exists::<NetworkTick>()
                //.run_if(client_connected)
                .label("client_apply_input_buffer"),
        );
    }
}

/// Deduplicate renet errors so we don't spam the logs with the same message.
#[derive(Resource, Debug)]
pub struct PreviousRenetError(Option<String>);

#[cfg(feature = "public")]
pub fn handle_renet_error(
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

/// Reset the networking state if the client was disconnected from the server so we can
/// try and reconnect in the future without weirdness like duplicate entities.
#[cfg(feature = "public")]
pub fn handle_client_disconnect(
    mut commands: Commands,
    local: Option<Res<crate::Local>>,
    tick: Option<Res<NetworkTick>>,
    client: Option<Res<RenetClient>>,
    server: Option<Res<RenetServer>>,
) {
    if local.is_some() {
        return;
    }

    if let Some(client) = client {
        let disconnected = client.disconnected();
        if let Some(reason) = disconnected {
            error!("client disconnected: {}", reason);
            commands.remove_resource::<RenetClient>();
            commands.remove_resource::<NetworkTick>();
        }
    } else {
        if server.is_none() && tick.is_some() {
            error!("server disconnected, removing tick");
            commands.remove_resource::<NetworkTick>();
        }
    }
}
