//! Who asks: the asking person and the people the provider knows as AIs (the directory of run 2b brings the real
//! source; until then the owner of the provider says so with `LocalTasks::mark_agent`).
//!
//! Pattern: Value object (one per call; the provider copies it out of its lock, so nothing is awaited under a lock).

use fs_tasks_testdata::tasks_v1::PersonRef;

/// A deliberate fault of the rights (mutants of the tests, never set otherwise): the rule must turn red. Only with the
/// feature `test-hooks`: the delivered provider has no way to switch its own protection off.
#[cfg(feature = "test-hooks")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flaw {
    /// An AI is not held at `Write` (its `Manage` grant works).
    NoAiCap,
    /// The grant rows are not read (the owner has `Manage` whatever the grant says, nobody else sees the list).
    GrantIgnored,
}

/// The asking person and the known AIs.
#[derive(Clone, Debug)]
pub struct Who {
    pub me: PersonRef,
    pub agents: Vec<PersonRef>,
    #[cfg(feature = "test-hooks")]
    pub flaw: Option<Flaw>,
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

    /// The deliberate fault "the grant rows are not read" is on (always `false` without `test-hooks`).
    #[must_use]
    pub fn grants_ignored(&self) -> bool {
        #[cfg(feature = "test-hooks")]
        return self.flaw == Some(Flaw::GrantIgnored);
        #[cfg(not(feature = "test-hooks"))]
        false
    }

    /// The deliberate fault "an AI is not held at `Write`" is on (always `false` without `test-hooks`).
    #[must_use]
    pub fn ai_uncapped(&self) -> bool {
        #[cfg(feature = "test-hooks")]
        return self.flaw == Some(Flaw::NoAiCap);
        #[cfg(not(feature = "test-hooks"))]
        false
    }
}
