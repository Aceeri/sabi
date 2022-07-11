use std::collections::BTreeMap;

use bevy::{ecs::entity::Entities, prelude::*};

use super::{NetworkTick, Replicate};

pub const SNAPSHOT_RETAIN_BUFFER: i64 = 32;

#[derive(Deref, DerefMut, Debug)]
pub struct ComponentSnapshot<C>(BTreeMap<Entity, C>);

impl<C> Default for ComponentSnapshot<C> {
    fn default() -> Self {
        Self(Default::default())
    }
}

#[derive(Debug)]
pub struct SnapshotBuffer<C> {
    snapshots: BTreeMap<NetworkTick, ComponentSnapshot<C>>,
}

impl<C> Default for SnapshotBuffer<C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C> SnapshotBuffer<C> {
    pub fn new() -> Self {
        Self {
            snapshots: Default::default(),
        }
    }

    pub fn push(&mut self, tick: NetworkTick, snapshot: ComponentSnapshot<C>) {
        self.snapshots.insert(tick, snapshot);

        self.clean_old();
    }

    pub fn clean_old(&mut self) {
        let newest = self.snapshots.keys().max().cloned().unwrap_or_default();

        self.snapshots.retain(|tick, _| {
            (newest.tick() as i64) - (tick.tick() as i64) < SNAPSHOT_RETAIN_BUFFER
        });
    }
}

pub fn store_snapshot<C>(
    tick: Res<NetworkTick>,
    mut snapshots: ResMut<SnapshotBuffer<C>>,
    query: Query<(Entity, &C)>,
) where
    C: 'static + Send + Sync + Component + Replicate + Clone,
{
    let mut snapshot = ComponentSnapshot::default();
    for (entity, component) in query.iter() {
        snapshot.insert(entity, component.clone());
    }

    snapshots.push(*tick, snapshot)
}

pub fn rewind<C>(
    mut commands: Commands,
    entities: &Entities,
    tick: Res<NetworkTick>,
    snapshots: Res<SnapshotBuffer<C>>,
) where
    C: 'static + Send + Sync + Component + Replicate + Clone,
{
    if let Some(snapshot) = snapshots.snapshots.get(&*tick) {
        for (entity, component) in snapshot.0.iter() {
            if entities.contains(*entity) {
                commands.entity(*entity).insert(component.clone());
            }
        }
    } else {
        /*
               error!(
                   "no snapshot for component: {:?}",
                   std::any::type_name::<C>()
               );
        */
    }
}
