use bevy::prelude::*;

use std::marker::PhantomData;

use crate::protocol::*;

pub const DEFAULT_ESTIMATE: usize = 128;

/// Try to guess what size the components that we are replicating will be.
///
/// We might want to do this the other way around where we serialize each component before
/// and then we combine each message so we know the definitive size before we queue
/// them up.
#[derive(Resource, Debug, Clone)]
pub struct ReplicateSizeEstimates(HashMap<ReplicateId, usize>);

impl ReplicateSizeEstimates {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn add(&mut self, id: ReplicateId, estimate: usize) {
        self.0.insert(id, estimate);
    }

    pub fn get(&self, id: &ReplicateId) -> usize {
        self.0.get(id).cloned().unwrap_or(DEFAULT_ESTIMATE)
    }
}

/// Maximum size in bytes for how long a replication request can be.
#[derive(Resource, Deref, Debug, Clone)]
pub struct ReplicateMaxSize(pub usize);

impl Default for ReplicateMaxSize {
    fn default() -> Self {
        Self(1500)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RequireDependency<ROOT, DEPENDENCY>(PhantomData<(ROOT, DEPENDENCY)>);

impl<ROOT, DEPENDENCY> RequireDependency<ROOT, DEPENDENCY> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<ROOT, DEPENDENCY> Default for RequireDependency<ROOT, DEPENDENCY> {
    fn default() -> Self {
        Self::new()
    }
}

impl<ROOT, DEPENDENCY> Plugin for RequireDependency<ROOT, DEPENDENCY>
where
    ROOT: 'static + Reflect + FromReflect,
    DEPENDENCY: 'static + Reflect + FromReflect,
{
    fn build(&self, app: &mut App) {
        if !app.world.contains_resource::<ReplicateDemands>() {
            app.world.init_resource::<ReplicateDemands>();
        }

        let mut demands = app
            .world
            .get_resource_mut::<ReplicateDemands>()
            .expect("replicate demands");

        demands
            .require
            .entry(crate::replicate_id::<ROOT>())
            .or_insert(Vec::new())
            .push(crate::replicate_id::<DEPENDENCY>())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RequireTogether<A, B>(RequireDependency<A, B>, RequireDependency<B, A>);

impl<A, B> RequireTogether<A, B> {
    pub fn new() -> Self {
        Self(RequireDependency::default(), RequireDependency::default())
    }
}

impl<A, B> Default for RequireTogether<A, B> {
    fn default() -> Self {
        Self::new()
    }
}

/// What components must be sent together and what can be left out if multiple are being sent.
///
/// This is mainly for saving bandwidth on stuff like sending both `Transform` and `GlobalTransform`
/// when it only makes sense to do so if they are both being sent.
#[derive(Resource, Debug, Default, Clone)]
pub struct ReplicateDemands {
    pub require: HashMap<ReplicateId, Vec<ReplicateId>>,
    pub dedup: HashMap<ReplicateId, Vec<ReplicateId>>,
}
