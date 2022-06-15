use bevy::{
    ecs::query::{FilterFetch, WorldQuery},
    prelude::*,
};

use smallvec::SmallVec;

use std::marker::PhantomData;

use crate::protocol::*;

pub type Priority = u16;

/// Priority accumulator across entities and components.
pub struct PriorityAccumulator {
    values: Vec<(Priority, Entity, ReplicateId)>,
    unused: Vec<usize>,
    needs_sort: bool,

    sorted: Vec<(Priority, Entity, ReplicateId)>,
    entity_map: HashMap<(Entity, ReplicateId), usize>,
}

impl PriorityAccumulator {
    pub fn new() -> Self {
        Self {
            values: Vec::new(),
            unused: Vec::new(),
            needs_sort: false,

            sorted: Vec::new(),
            entity_map: HashMap::new(),
        }
    }

    pub fn needs_sort(&mut self) {
        self.needs_sort = true;
    }

    pub fn get_or_insert_index(&mut self, entity: Entity, replicate_id: ReplicateId) -> usize {
        match self.entity_map.entry((entity, replicate_id)) {
            Entry::Occupied(occupied) => *occupied.get(),
            Entry::Vacant(_vacant) => self.new_index(entity, replicate_id),
        }
    }

    pub fn new_index(&mut self, entity: Entity, replicate_id: ReplicateId) -> usize {
        if let Some(unused) = self.unused.pop() {
            self.entity_map.insert((entity, replicate_id), unused);
            unused
        } else {
            self.needs_sort();
            self.values.push((Priority::MIN, entity, replicate_id));
            let index = self.values.len() - 1;
            self.entity_map.insert((entity, replicate_id), index);
            index
        }
    }

    pub fn clear(&mut self, entity: Entity, replicate_id: ReplicateId) {
        self.needs_sort();
        let index = self.get_or_insert_index(entity, replicate_id);
        self.values[index] = (Priority::MIN, entity, replicate_id);
    }

    pub fn bump(&mut self, entity: Entity, replicate_id: ReplicateId, priority: Priority) {
        self.needs_sort();
        let index = self.get_or_insert_index(entity, replicate_id);
        self.values[index].0 = self.values[index].0.saturating_add(priority);
    }

    pub fn clean(&mut self, entities: &Entities) {
        self.needs_sort();

        let mut mark = Vec::new();
        for (entity, replicate_id) in self.entity_map.keys() {
            if !entities.contains(*entity) {
                mark.push((*entity, *replicate_id));
            }
        }

        for index in mark {
            let index = self
                .entity_map
                .remove(&index)
                .expect("marked entity was not in map");

            self.values[index].0 = Priority::MIN;
            self.unused.push(index);
        }
    }

    pub fn update_sorted(&mut self) {
        self.sorted = self.values.clone();
        self.sorted
            .sort_by(|(a, _, _), (b, _, _)| b.partial_cmp(a).unwrap());
        self.needs_sort = false;
    }

    pub fn sorted(&mut self) -> &Vec<(Priority, Entity, ReplicateId)> {
        if self.needs_sort {
            self.update_sorted()
        }

        &self.sorted
    }
}

#[derive(Deref, DerefMut, Debug, Clone)]
pub struct ComponentsToSend(Vec<(Entity, ReplicateId)>);

impl ComponentsToSend {
    pub fn new() -> Self {
        Self(Vec::new())
    }
}

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
pub struct ReplicateMaxSize(usize);

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

/// Queue up components that we need to send.
pub fn fetch_top_priority(
    mut priority: ResMut<PriorityAccumulator>,
    demands: Res<ReplicateDemands>,
    estimates: Res<ReplicateSizeEstimates>,
    max: Res<ReplicateMaxSize>,
    mut to_send: ResMut<ComponentsToSend>,
) {
    let sorted = priority.sorted();

    to_send.clear();
    let mut used = 0usize;

    for (_priority, entity, replicate_id) in sorted {
        let mut grouped_ids: SmallVec<[&ReplicateId; 3]> = SmallVec::new();
        grouped_ids.push(replicate_id);
        if let Some(group) = demands.require.get(replicate_id) {
            grouped_ids.extend(group);
        }

        let estimate: usize = grouped_ids.iter().map(|id| estimates.get(id)).sum();

        let space_left = max.0.saturating_sub(used + estimate);
        if used + estimate > max.0 {
            if space_left > 30 {
                // Try to find another component that will fit that is somewhat lower priority.
                continue;
            } else {
                // We have used up our conservative estimated amount of bandwidth we can send
                break;
            }
        }

        for id in grouped_ids {
            to_send.push((*entity, *id));
        }

        used += estimate;
    }
}

pub fn server_bump_filtered<C, F, const BUMP: Priority>(
    mut priority: ResMut<PriorityAccumulator>,
    interest: Query<Entity, F>,
) where
    C: 'static + Send + Sync + Component + Replicate + Clone,
    F: WorldQuery,
    F::Fetch: FilterFetch,
{
    for entity in interest.iter() {
        priority.bump(entity, C::replicate_id(), BUMP);
    }
}
