//! The claim: `claim`, `release`, `claim_next` and "STOP KI".
//!
//! Pattern: Command methods of the state. Order of the checks (`tasks_v1/api.rs`): unknown or unreadable `NotFound`,
//! then the claim rule `Forbidden { task.claim }`, then the holder `Claimed`; an expired claim holds nobody. One
//! person holds at most; the assignees never change through a claim.

use crate::mutation::Mutation;
use crate::ready::is_done;
use crate::series::current;
use crate::state::{State, check};
use crate::tasks_v1::{
    Claim, Error, HistoryKind, ObjectId, PersonRef, Progress, Readiness, Revision, Task, TaskError,
    TaskQuery, TaskResult, Trash, Utc,
};

fn forbidden() -> TaskError {
    Error::Forbidden {
        capability: "task.claim".to_owned(),
    }
    .into()
}

impl State {
    /// The claim that still holds another person (an expired one holds nobody).
    pub(crate) fn held_by_other(&self, i: usize) -> Option<Claim> {
        let c = self.rows[i].task.claim.clone()?;
        ((c.until > self.now || self.mutated(Mutation::ExpiredHolds)) && c.by.id != self.asker().id)
            .then_some(c)
    }

    fn take(&mut self, i: usize, until: Utc) {
        self.rows[i].task.claim = Some(Claim {
            by: self.asker(),
            until,
        });
        self.bump(i, HistoryKind::Join, Vec::new());
    }

    pub(crate) fn claim(
        &mut self,
        id: ObjectId,
        expected: Revision,
        until: Utc,
    ) -> TaskResult<Task> {
        self.expire();
        let i = self.live(id)?;
        if self.mutated(Mutation::ClaimedFirst)
            && let Some(c) = self.held_by_other(i)
        {
            return Err(TaskError::Claimed(c));
        }
        if !self.may_claim(&self.rows[i].task) {
            return Err(forbidden());
        }
        check(self.rows[i].task.revision, expected)?;
        if is_done(self.rows[i].task.progress) {
            return Err(Self::bad("task", "finished"));
        }
        if until <= self.now {
            return Err(Self::bad("until", "past"));
        }
        if let Some(c) = self.held_by_other(i) {
            return Err(TaskError::Claimed(c));
        }
        self.take(i, until);
        Ok(self.rows[i].task.clone())
    }

    pub(crate) fn release(&mut self, id: ObjectId, expected: Revision) -> TaskResult<Task> {
        self.expire();
        let i = self.live(id)?;
        let mine = self.rows[i]
            .task
            .claim
            .as_ref()
            .is_some_and(|c| c.by.id == self.asker().id);
        let open = self.mutated(Mutation::ReleaseOpen) && self.held_by_other(i).is_none();
        if !mine && !open && !self.may_claim(&self.rows[i].task) {
            return Err(forbidden());
        }
        check(self.rows[i].task.revision, expected)?;
        if let Some(c) = self.held_by_other(i) {
            return Err(TaskError::Claimed(c));
        }
        if self.rows[i].task.claim.take().is_some() {
            self.bump(i, HistoryKind::Leave, Vec::new());
        }
        Ok(self.rows[i].task.clone())
    }

    /// The first task in the order of `query` that is open, free, claimable by the asking person and (with
    /// `dependency.evaluate`) ready; without evaluation a task with dependencies is skipped. One transaction.
    pub(crate) fn claim_next(&mut self, q: &TaskQuery, until: Utc) -> TaskResult<Option<Task>> {
        self.expire();
        if q.ready.is_some() || q.trash != Trash::Exclude {
            return Err(Self::bad("query", "ready-or-trash"));
        }
        let rows = self.selected(q)?;
        if until <= self.now {
            return Err(Self::bad("until", "past"));
        }
        let found = rows.into_iter().find(|&i| {
            let t = &self.rows[i].task;
            let open = matches!(
                current(t).progress,
                Progress::NeedsAction | Progress::InProcess
            );
            let ready = if self.mutated(Mutation::NextIgnoresReady) {
                true
            } else if self.evaluate {
                self.readiness_of(i) == Readiness::Ready
            } else {
                t.dependencies.is_empty()
            };
            open && ready && self.held_by_other(i).is_none() && self.may_claim(t)
        });
        Ok(found.map(|i| {
            self.take(i, until);
            self.rows[i].task.clone()
        }))
    }

    /// "STOP KI": the claims of `who` end at once.
    pub(crate) fn revoke_claims(&mut self, who: &PersonRef) {
        for i in 0..self.rows.len() {
            if self.rows[i]
                .task
                .claim
                .as_ref()
                .is_some_and(|c| c.by.id == who.id)
            {
                self.rows[i].task.claim = None;
                self.bump(i, HistoryKind::Leave, Vec::new());
            }
        }
    }
}
