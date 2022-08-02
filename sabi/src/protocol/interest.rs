use std::{
    collections::{BTreeMap, VecDeque},
    fmt::Debug,
    hash::Hash,
};

use bevy_renet::renet::ServerEvent;
use smallvec::SmallVec;

use bevy::{prelude::*, utils::HashSet};

use super::{
    demands::{ReplicateDemands, ReplicateMaxSize, ReplicateSizeEstimates},
    ClientId, NetworkTick, Replicate, ReplicateId,
};

pub const RESEND_INTEREST_BUFFER: i64 = 32;

pub type Interest = (Entity, ReplicateId);

#[derive(Debug, Clone, Default)]
pub struct Baseload {
    clients: BTreeMap<ClientId, bool>,
}

impl Baseload {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mark(&mut self, client_id: ClientId) {
        let should = self.clients.entry(client_id).or_default();
        *should = true;
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ClientId, &bool)> {
        self.clients.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&ClientId, &mut bool)> {
        self.clients.iter_mut()
    }
}

pub fn setup_baseload(mut baseload: ResMut<Baseload>, mut server_events: EventReader<ServerEvent>) {
    for event in server_events.iter() {
        match event {
            ServerEvent::ClientConnected(client_id, _user_data) => {
                info!("baseloading {}", client_id);
                baseload.mark(*client_id);
            }
            _ => {}
        }
    }
}

pub fn baseload_components<C>(
    mut baseload: ResMut<Baseload>,
    mut queues: ResMut<ClientInterestQueues>,
    query: Query<Entity, With<C>>,
) where
    C: 'static + Send + Sync + Component + Replicate + Clone,
{
    for (client_id, should_load) in baseload.iter_mut() {
        if *should_load {
            let queue = queues.entry(*client_id);
            for interest in query.iter().map(|e| (e, <C as Replicate>::replicate_id())) {
                queue.push_back(interest);
            }
        }
    }
}

pub fn clear_baseloads(mut baseload: ResMut<Baseload>) {
    for (_client_id, should_load) in baseload.iter_mut() {
        //*should_load = false;
    }
}

pub fn component_changes<C>(
    mut queues: ResMut<ClientInterestQueues>,
    query: Query<Entity, Changed<C>>,
) where
    C: 'static + Send + Sync + Component + Replicate + Clone,
{
    let changes = query
        .iter()
        .map(|e| (e, <C as Replicate>::replicate_id()))
        .collect::<Vec<_>>();

    for (_client_id, queue) in queues.iter_mut() {
        for change in changes.iter() {
            queue.push_back(change.clone());
        }
    }
}

/// Sent clients interests for this frame.
#[derive(Default, Debug, Clone)]
pub struct ClientUnackedInterests {
    clients: BTreeMap<ClientId, UnackedInterests>,
}

impl ClientUnackedInterests {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, client_id: ClientId, tick: NetworkTick, interests: Vec<Interest>) {
        self.clients
            .entry(client_id)
            .or_default()
            .record(tick, interests);
    }

    pub fn record_from_queue(&mut self, tick: NetworkTick, queue: &InterestsToSend) {
        for (client_id, interests) in queue.iter() {
            self.record(*client_id, tick, interests.clone());
        }
    }

    pub fn ack(&mut self, client_id: &ClientId, tick: &NetworkTick) {
        if let Some(sent) = self.clients.get_mut(client_id) {
            sent.ack(tick);
        }
    }

    pub fn resend_unacked(&mut self, tick: NetworkTick, queues: &mut ClientInterestQueues) {
        for (client_id, sent) in &mut self.clients {
            let queue = queues.entry(*client_id);
            sent.resend_unacked(tick, queue);
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ClientId, &UnackedInterests)> {
        self.clients.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&ClientId, &mut UnackedInterests)> {
        self.clients.iter_mut()
    }
}

#[derive(Default, Debug, Clone)]
pub struct UnackedInterests {
    unacked: BTreeMap<NetworkTick, Vec<Interest>>,
}

impl UnackedInterests {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, tick: NetworkTick, interests: Vec<Interest>) {
        self.unacked.entry(tick).or_default().extend(interests);
    }

    pub fn ack(&mut self, tick: &NetworkTick) {
        self.unacked.remove(tick);
    }

    pub fn resend_unacked(
        &mut self,
        current_tick: NetworkTick,
        queue: &mut InterestQueue<Interest>,
    ) {
        let mut resend = Vec::new();
        for (tick, interests) in self.unacked.iter().filter(|(tick, _)| {
            (current_tick.tick() as i64 - tick.tick() as i64) < RESEND_INTEREST_BUFFER
        }) {
            for interest in interests.iter() {
                queue.push_front(*interest);
            }

            resend.push(*tick);
        }

        for tick in resend {
            self.unacked.remove(&tick);
        }
    }
}

pub fn resend_unacked(
    tick: Res<NetworkTick>,
    mut unacked: ResMut<ClientUnackedInterests>,
    mut queues: ResMut<ClientInterestQueues>,
) {
    unacked.resend_unacked(*tick, &mut *queues);
}

/// Queue up components that we need to send.
pub fn queue_interests(
    tick: Res<NetworkTick>,
    mut queues: ResMut<ClientInterestQueues>,
    demands: Res<ReplicateDemands>,
    estimates: Res<ReplicateSizeEstimates>,
    max: Res<ReplicateMaxSize>,
    mut to_send: ResMut<InterestsToSend>,
    mut sent_unacked: ResMut<ClientUnackedInterests>,
) {
    to_send.clear();

    for (client_id, queue) in queues.iter_mut() {
        let mut used = 0usize;
        let mut unsent: SmallVec<[Interest; 3]> = SmallVec::new();

        while let Some((entity, replicate_id)) = queue.pop_front() {
            //info!("attempting: ({:?}, {:?})", entity, replicate_id);
            let mut grouped_ids: SmallVec<[&ReplicateId; 3]> = SmallVec::new();
            grouped_ids.push(&replicate_id);
            if let Some(group) = demands.require.get(&replicate_id) {
                grouped_ids.extend(group);
            }

            let estimate: usize = grouped_ids.iter().map(|id| estimates.get(id)).sum();

            let space_left = max.0.saturating_sub(used + estimate);

            if used + estimate > max.0 {
                // need to be careful to not lose any updates
                // so we store the one we popped in a temp vec
                unsent.push((entity, replicate_id));

                if space_left > 30 {
                    // Try to find another component that will fit that is somewhat lower priority.
                    continue;
                } else {
                    // We have used up our conservative estimated amount of bandwidth we can send
                    break;
                }
            }

            for id in grouped_ids {
                to_send.push(*client_id, (entity, *id));
            }

            used += estimate;
        }

        for interest in unsent.into_iter().rev() {
            //info!("unsent, repushing: {:?}", interest);
            queue.push_front(interest);
        }
    }

    sent_unacked.record_from_queue(*tick, &*to_send);
}

#[derive(Default, Clone)]
pub struct ClientInterestQueues {
    queues: BTreeMap<ClientId, InterestQueue<Interest>>,
}

impl ClientInterestQueues {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ClientId, &InterestQueue<Interest>)> {
        self.queues.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&ClientId, &mut InterestQueue<Interest>)> {
        self.queues.iter_mut()
    }

    pub fn get(&self, client_id: &ClientId) -> Option<&InterestQueue<Interest>> {
        self.queues.get(client_id)
    }

    pub fn get_mut(&mut self, client_id: &ClientId) -> Option<&mut InterestQueue<Interest>> {
        self.queues.get_mut(client_id)
    }

    pub fn entry(&mut self, client_id: ClientId) -> &mut InterestQueue<Interest> {
        self.queues.entry(client_id).or_default()
    }
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
    queue.push_back(1i32);
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
    let empty_slice: &[i32] = &[];
    assert_eq!(
        queue.iter().cloned().collect::<Vec<_>>().as_slice(),
        empty_slice
    );
    assert_eq!(queue.pop_front(), None);
    assert_eq!(
        queue.iter().cloned().collect::<Vec<_>>().as_slice(),
        empty_slice
    );
}

#[derive(Default, Debug, Clone)]
pub struct InterestsToSend {
    clients: BTreeMap<ClientId, Vec<Interest>>,
}

impl InterestsToSend {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ClientId, &Vec<Interest>)> {
        self.clients.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&ClientId, &mut Vec<Interest>)> {
        self.clients.iter_mut()
    }

    pub fn push(&mut self, client_id: ClientId, interest: Interest) {
        self.clients.entry(client_id).or_default().push(interest);
    }

    pub fn clear(&mut self) {
        for (_client_id, interests) in self.iter_mut() {
            interests.clear();
        }
    }
}
