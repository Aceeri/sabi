use std::{
    collections::{BTreeMap, VecDeque},
    fmt::Debug,
    hash::Hash,
    marker::PhantomData,
};

use smallvec::SmallVec;

use bevy::{prelude::*, utils::HashSet};

use super::{
    priority::{ComponentsToSend, ReplicateDemands, ReplicateMaxSize, ReplicateSizeEstimates},
    ClientId, NetworkTick, Replicate, ReplicateId,
};

pub type Interest = (Entity, ReplicateId);

/// Per component tracking of changes.
pub struct InterestChanges {
    tick: NetworkTick,
    changes: Vec<Interest>,
}

pub fn clear_component_changes(tick: Res<NetworkTick>, mut list: ResMut<InterestChanges>) {
    list.changes.clear();
    list.tick = *tick;
}

pub fn component_changes<C>(
    tick: Res<NetworkTick>,
    mut list: ResMut<InterestChanges>,
    query: Query<Entity, Changed<C>>,
) where
    C: 'static + Send + Sync + Component + Replicate + Clone,
{
    assert_eq!(list.tick, *tick, "we didn't clear the component changes");
    list.changes
        .extend(query.iter().map(|e| (e, <C as Replicate>::replicate_id())));
}

/// Clients interests for this frame.
#[derive(Default, Clone)]
pub struct SentInterests {
    sent: BTreeMap<NetworkTick, Vec<(Entity, ReplicateId)>>,
}

impl SentInterests {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Queue up components that we need to send.
pub fn send_interests(
    mut queues: ResMut<ClientInterestQueues>,
    demands: Res<ReplicateDemands>,
    estimates: Res<ReplicateSizeEstimates>,
    max: Res<ReplicateMaxSize>,
    mut to_send: ResMut<ComponentsToSend>,
) {
    to_send.clear();
    let mut used = 0usize;

    for (client, queue) in queues.iter() {
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
}

#[derive(Default, Clone)]
pub struct ClientInterestQueues {
    queues: BTreeMap<ClientId, InterestQueue<(Entity, ReplicateId)>>,
}

#[derive(Debug, Clone)]
pub struct InterestQueue<I>
where
    I: PartialEq + Eq + PartialOrd + Ord + Hash + Clone + Debug,
{
    contains: HashSet<I>,
    queue: VecDeque<I>,
}

impl<I> Default for InterestQueue<I>
where
    I: PartialEq + Eq + PartialOrd + Ord + Hash + Clone + Debug,
{
    fn default() -> Self {
        Self {
            contains: Default::default(),
            queue: Default::default(),
        }
    }
}

impl<I> InterestQueue<I>
where
    I: PartialEq + Eq + PartialOrd + Ord + Hash + Clone + Debug,
{
    pub fn new() -> Self {
        Self::default()
    }

    /// Push an interest to the back of the queue, returns true if it was already in.
    pub fn push_back(&mut self, interest: I) -> bool {
        let contains = self.contains.contains(&interest);

        if !contains {
            self.contains.insert(interest.clone());
            self.queue.push_back(interest);
        }

        contains
    }

    /// Push an interest to the front of the queue
    ///
    /// If it is already in queue then it will be moved forward.
    pub fn push_front(&mut self, interest: I) -> bool {
        let key = interest;
        let contains = self.contains.contains(&key);

        if !contains {
            self.contains.insert(key.clone());
            self.queue.push_front(key);
        } else {
            let index = self
                .queue
                .iter()
                .enumerate()
                .find(|(_, k)| **k == key)
                .map(|(index, _)| index)
                .expect("contains set has key but isn't in the queue");
            self.queue.swap_remove_back(index);
            self.queue.push_front(key);
        }

        contains
    }

    /// Pop the next entity/component pair from the front.
    pub fn pop_front(&mut self) -> Option<I> {
        if let Some(key) = self.queue.pop_front() {
            self.contains.remove(&key);
            Some(key)
        } else {
            None
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &I> {
        self.queue.iter()
    }

    pub fn peek_first(&self) -> Option<&I> {
        self.iter().next()
    }

    pub fn peek_last(&self) -> Option<&I> {
        self.iter().last()
    }
}

#[test]
pub fn interest_queue() {
    let mut queue = InterestQueue::new();
    queue.push_back(1);
    queue.push_back(2);
    queue.push_back(2);
    queue.push_back(3);

    assert_eq!(queue.queue.len(), 3); // should dedup the 2s

    // should re-order if asked to push to front
    assert_eq!(queue.peek_last(), Some(&3));
    queue.push_front(3);
    assert_eq!(queue.peek_first(), Some(&3));
    assert_eq!(queue.peek_last(), Some(&2));

    assert_eq!(
        queue.iter().cloned().collect::<Vec<_>>().as_slice(),
        &[3, 1, 2]
    );
    assert_eq!(queue.pop_front(), Some(3));
    assert_eq!(
        queue.iter().cloned().collect::<Vec<_>>().as_slice(),
        &[1, 2]
    );

    assert_eq!(queue.pop_front(), Some(1));
    assert_eq!(queue.iter().cloned().collect::<Vec<_>>().as_slice(), &[2]);

    assert_eq!(queue.pop_front(), Some(2));
    assert_eq!(queue.iter().cloned().collect::<Vec<_>>().as_slice(), &[]);
    assert_eq!(queue.pop_front(), None);
    assert_eq!(queue.iter().cloned().collect::<Vec<_>>().as_slice(), &[]);
}

#[derive(Default, Debug, Clone)]
pub struct InterestToSend {
    clients: BTreeMap<ClientId, Vec<Interest>>,
}

impl ComponentsToSend {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ClientId, &Vec<Interest>)> {
        self.clients.iter()
    }

    pub fn iter_mut(&self) -> impl Iterator<Item = (&mut ClientId, &mut Vec<Interest>)> {
        self.clients.iter_mut()
    }

    pub fn clear(&mut self) {
        for (client, interests) in self.iter_mut() {
            interests.clear();
        }
    }
}
