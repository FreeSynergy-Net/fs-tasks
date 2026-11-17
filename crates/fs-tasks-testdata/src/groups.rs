//! `GroupMembers` of the fake: who is in which group, who manages it, who is an AI (the consumed side of fs-groups and
//! fs-people, handed to the provider in its constructor and never stored in a task).
//!
//! Pattern: Port + Fake (the narrow view the tasks provider needs; the real one is `GroupMembers::memberships_of` of
//! fs-groups `groups_v1/provided.rs` plus the AI mark of the `PersonDirectory` of fs-people; this local trait keeps the
//! same names until fs-groups offers its crate at a tag, then `LocalTasks` takes the real trait and this one goes).

use crate::ids::person;
use crate::tasks_v1::PersonRef;
use std::fmt::Debug;
use std::sync::{Arc, Mutex, MutexGuard};

/// The questions the tasks provider asks about groups and people (always at the time of the question, never stored).
pub trait GroupMembers: Send + Sync + Debug {
    /// The groups the person is an effective member of (`GroupMembers::memberships_of`; the provider asks THIS, never
    /// "who are the members of group G", so a sensitive group gives nothing away to an AI).
    fn memberships_of(&self, person: &PersonRef) -> Vec<PersonRef>;
    /// Whether the person may manage the group (`groups_v1`: administrators of the group).
    fn manages(&self, person: &PersonRef, group: &PersonRef) -> bool;
    /// Whether the person or group is an AI or an AI group (the AI mark of the directory).
    fn is_agent(&self, who: &PersonRef) -> bool;
}

#[derive(Debug, Default)]
struct Book {
    members: Vec<(PersonRef, PersonRef)>,
    managers: Vec<(PersonRef, PersonRef)>,
    agents: Vec<PersonRef>,
}

/// An in-memory `GroupMembers`; people are compared by their IRI, not by their kind.
#[derive(Debug, Default)]
pub struct FakeGroupMembers {
    book: Mutex<Book>,
}

fn same(a: &PersonRef, b: &PersonRef) -> bool {
    a.id == b.id
}

impl FakeGroupMembers {
    /// No groups, no AI.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The groups of the sample people ([`crate::KNOWN`]): "Team Vorstand" (6) has 1, 2 and 3, "Team Umzug" (7) has 2
    /// and 4 (the acting person is not in it), the AI "Hilfe-Assistent" (8) is alone in the AI group "KI Helfer" (9);
    /// person 1 manages group 6.
    #[must_use]
    pub fn sample() -> Self {
        let this = Self::new();
        let kind = |n: u64| {
            crate::sample_people::KNOWN
                .iter()
                .find(|k| k.number == n)
                .map_or_else(|| person(n), crate::Known::person)
        };
        for (member, group) in [(1, 6), (2, 6), (3, 6), (2, 7), (4, 7), (8, 9)] {
            this.add_member(&kind(member), &kind(group));
        }
        this.add_manager(&kind(1), &kind(6));
        this.mark_agent(&kind(8));
        this.mark_agent(&kind(9));
        this
    }

    fn lock(&self) -> MutexGuard<'_, Book> {
        self.book
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// `member` is an effective member of `group`.
    pub fn add_member(&self, member: &PersonRef, group: &PersonRef) {
        self.lock().members.push((member.clone(), group.clone()));
    }

    /// `member` leaves `group`.
    pub fn remove_member(&self, member: &PersonRef, group: &PersonRef) {
        self.lock()
            .members
            .retain(|(m, g)| !(same(m, member) && same(g, group)));
    }

    /// `manager` may manage `group`.
    pub fn add_manager(&self, manager: &PersonRef, group: &PersonRef) {
        self.lock().managers.push((manager.clone(), group.clone()));
    }

    /// `who` (a person or a group) is an AI.
    pub fn mark_agent(&self, who: &PersonRef) {
        self.lock().agents.push(who.clone());
    }

    /// Shared handle for the constructor of a provider.
    #[must_use]
    pub fn shared(self) -> Arc<dyn GroupMembers> {
        Arc::new(self)
    }
}

impl GroupMembers for FakeGroupMembers {
    fn memberships_of(&self, who: &PersonRef) -> Vec<PersonRef> {
        let book = self.lock();
        book.members
            .iter()
            .filter(|(m, _)| same(m, who))
            .map(|(_, g)| g.clone())
            .collect()
    }

    fn manages(&self, who: &PersonRef, group: &PersonRef) -> bool {
        let book = self.lock();
        book.managers
            .iter()
            .any(|(m, g)| same(m, who) && same(g, group))
    }

    fn is_agent(&self, who: &PersonRef) -> bool {
        self.lock().agents.iter().any(|a| same(a, who))
    }
}
