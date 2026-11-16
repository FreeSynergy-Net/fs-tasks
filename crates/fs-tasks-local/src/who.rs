//! Who asks: the asking person and the people the provider knows as AIs (the directory of run 2b brings the real
//! source; until then the owner of the provider says so with `LocalTasks::mark_agent`).
//!
//! Pattern: Value object (one per call; the provider copies it out of its lock, so nothing is awaited under a lock).

use fs_tasks_testdata::tasks_v1::PersonRef;

/// The asking person and the known AIs.
#[derive(Clone, Debug)]
pub struct Who {
    pub me: PersonRef,
    pub agents: Vec<PersonRef>,
}

impl Who {
    /// Whether `p` is an AI.
    #[must_use]
    pub fn is_agent(&self, p: &PersonRef) -> bool {
        self.agents.iter().any(|a| a.id == p.id)
    }

    /// Whether the asking person is an AI (no list of its own, at most `Write`).
    #[must_use]
    pub fn asks_as_agent(&self) -> bool {
        self.is_agent(&self.me)
    }
}
