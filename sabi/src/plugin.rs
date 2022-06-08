use std::marker::PhantomData;

use bevy::prelude::*;
use bevy_renet::{
    renet::{RenetError, RenetServer},
    run_if_client_conected, RenetClientPlugin,
};
use iyes_loopless::prelude::{ConditionHelpers, IntoConditionalSystem};

use crate::{
    protocol::updates::{EntityUpdate, Reliable, Unreliable},
    replicate::physics::ReplicatePhysicsPlugin,
    Replicate,
};

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
        app.add_system(
            crate::protocol::server_queue_interest_reliable::<C>
                .run_if(crate::protocol::on_network_tick)
                .run_if_resource_exists::<RenetServer>()
                .before("send_interests"),
        );

        app.add_system(
            crate::protocol::client_update_reliable::<C>.with_run_criteria(run_if_client_conected),
        );
    }
}

pub struct SabiPlugin;

impl Plugin for SabiPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<ServerEntity>();

        app.add_event::<(ServerEntity, ComponentsUpdate)>();

        app.insert_resource(ServerEntities::default());
        app.insert_resource(Reliable::<EntityUpdate>(EntityUpdate::new()));
        app.insert_resource(Unreliable::<EntityUpdate>(EntityUpdate::new()));

        app.insert_resource(Lobby::default());
        app.insert_resource(NetworkGameTimer::default());

        app.add_plugin(ReplicatePhysicsPlugin);

        app.add_system(tick_network);

        app.add_plugin(ReplicatePlugin::<Transform>::default());
        app.add_plugin(ReplicatePlugin::<GlobalTransform>::default());
        app.add_plugin(ReplicatePlugin::<Name>::default());
    }
}

pub struct SabiServerPlugin;

impl Plugin for SabiServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(SabiPlugin);

        app.insert_resource(crate::protocol::new_renet_server());

        app.add_plugin(bevy_renet::RenetServerPlugin);

        app.add_system(
            server_send_interest_reliable
                .run_if_resource_exists::<RenetServer>()
                .run_if(on_network_tick)
                .label("send_interests"),
        );

        app.add_system(
            server_clear_reliable_queue
                .run_if(on_network_tick)
                .after("send_interests"),
        );
        app.add_system(log_on_error_system);
    }
}

pub struct SabiClientPlugin;

impl Plugin for SabiClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(SabiPlugin);

        app.insert_resource(new_renet_client());
        app.add_plugin(RenetClientPlugin);

        app.add_system(client_recv_interest_reliable.with_run_criteria(run_if_client_conected));
        app.add_system(client_update_reliable::<Transform>);
        app.add_system(client_update_reliable::<GlobalTransform>);
        app.add_system(client_update_reliable::<Name>);

        app.insert_resource(PreviousRenetError(None));
        app.add_system(log_on_error_system);
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
