use std::{marker::PhantomData, time::Duration};

use bevy::prelude::*;
use bevy_renet::{
    renet::{RenetClient, RenetError, RenetServer},
    run_if_client_conected, RenetClientPlugin,
};
use iyes_loopless::prelude::{ConditionHelpers, IntoConditionalSystem};

use crate::{
    protocol::update::EntityUpdate, replicate::physics::ReplicatePhysicsPlugin, Replicate,
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

impl<C> Plugin for ReplicatePlugin<C>
where
    C: 'static + Send + Sync + Component + Replicate + Clone,
{
    fn build(&self, app: &mut App) {
        if app.world.contains_resource::<RenetServer>() {
            app.add_system(
                crate::protocol::update::server_queue_interest::<C>
                    .run_if(crate::protocol::on_network_tick)
                    .run_if_resource_exists::<RenetServer>()
                    .before("send_interests")
                    .after("fetch_priority"),
            );

            app.add_system(
                crate::protocol::priority::server_bump_all::<C>
                    .run_if(crate::protocol::on_network_tick)
                    .run_if_resource_exists::<RenetServer>()
                    .before("fetch_priority"),
            );

            app.add_system(
                crate::protocol::priority::server_bump_changed::<C>
                    .run_if(crate::protocol::on_network_tick)
                    .run_if_resource_exists::<RenetServer>()
                    .before("fetch_priority"),
            );
        }

        if app.world.contains_resource::<RenetClient>() {
            app.add_system(
                crate::protocol::update::client_update_reliable::<C>
                    .with_run_criteria(run_if_client_conected),
            );
        }
    }
}

#[derive(Debug, Clone)]
pub struct SabiPlugin {
    pub tick_rate: Duration,
}

impl Default for SabiPlugin {
    fn default() -> Self {
        Self {
            tick_rate: tick_hz(32),
        }
    }
}

impl Plugin for SabiPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<ServerEntity>();

        app.add_event::<(ServerEntity, ComponentsUpdate)>();

        app.insert_resource(ServerEntities::default());
        app.insert_resource(EntityUpdate::new());

        app.insert_resource(Lobby::default());
        app.insert_resource(NetworkGameTimer::new(self.tick_rate));

        app.add_plugin(ReplicatePhysicsPlugin);

        app.add_system(tick_network);

        app.add_plugin(ReplicatePlugin::<Transform>::default());
        app.add_plugin(ReplicatePlugin::<GlobalTransform>::default());
        app.add_plugin(ReplicatePlugin::<Name>::default());

        app.insert_resource(PreviousRenetError(None));
        app.add_system(log_on_error_system);
    }
}

pub struct SabiServerPlugin;

impl Plugin for SabiServerPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(crate::protocol::new_renet_server());
        app.insert_resource(crate::protocol::priority::PriorityAccumulator::new());
        app.insert_resource(crate::protocol::priority::ReplicateSizeEstimates::new());
        app.insert_resource(crate::protocol::priority::ReplicateMaxSize::default());
        app.insert_resource(crate::protocol::priority::ComponentsToSend::new());

        app.add_plugin(bevy_renet::RenetServerPlugin);

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

        app.insert_resource(BandwidthTimer::new());
        app.add_system(display_server_bandwidth);

        app.add_system(
            crate::protocol::update::server_clear_queue
                .run_if(on_network_tick)
                .after("send_interests"),
        );
        app.add_system(log_on_error_system);
    }
}

pub struct SabiClientPlugin;

impl Plugin for SabiClientPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(new_renet_client());
        app.add_plugin(RenetClientPlugin);

        app.add_system(
            crate::protocol::update::client_recv_interest_reliable
                .with_run_criteria(run_if_client_conected),
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
