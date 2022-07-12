use std::{
    collections::{BTreeMap, VecDeque},
    fmt::Debug,
    hash::Hash,
    marker::PhantomData,
};

use bevy::{prelude::Entity, utils::HashSet};

use super::{ack::NetworkAck, ClientId, NetworkTick, ReplicateId};

pub type Interest = (Entity, ReplicateId);

/// Per component tracking of changes.
pub struct InterestChanges {
    changes: BTreeMap<NetworkTick, Vec<Interest>>,
}

/// Per component tracking of changes.
pub struct ComponentChanges<C> {
    changes: Vec<Entity>,
    phantom: PhantomData<C>,
}

/// Clients interests for this frame.
pub struct SentInterests {
    sent: BTreeMap<NetworkTick, Vec<(Entity, ReplicateId)>>,
}

impl SentInterests {
    pub fn new() -> Self {
        Self {
            sent: Default::default(),
        }
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

impl<I> InterestQueue<I>
where
    I: PartialEq + Eq + PartialOrd + Ord + Hash + Clone + Debug,
{
    pub fn new() -> Self {
        Self {
            contains: HashSet::new(),
            queue: Default::default(),
        }
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
