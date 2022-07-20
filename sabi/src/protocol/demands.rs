use bevy::prelude::*;

use std::marker::PhantomData;

use crate::protocol::*;

#[derive(Debug, Clone)]
pub struct ReplicateSizeEstimates(HashMap<ReplicateId, usize>);

impl ReplicateSizeEstimates {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn add(&mut self, id: ReplicateId, estimate: usize) {
        self.0.insert(id, estimate);
    }

    pub fn get(&self, id: &ReplicateId) -> usize {
        self.0.get(id).cloned().unwrap_or(0)
    }
}

#[derive(Deref, Debug, Clone)]
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
    ROOT: Replicate,
    DEPENDENCY: Replicate,
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
            .entry(ROOT::replicate_id())
            .or_insert(Vec::new())
            .push(DEPENDENCY::replicate_id())
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
#[derive(Debug, Default, Clone)]
pub struct ReplicateDemands {
    pub require: HashMap<ReplicateId, Vec<ReplicateId>>,
    pub dedup: HashMap<ReplicateId, Vec<ReplicateId>>,
}
