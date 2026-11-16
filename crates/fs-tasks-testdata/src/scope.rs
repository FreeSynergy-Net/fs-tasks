//! Series scope: which stored thing a write with a [`SeriesScope`] acts on.
//!
//! Pattern: Strategy (`Whole` acts on the task, `This` on the override of one occurrence, `ThisAndFollowing`
//! splits the series first and then acts on the new task).

use crate::series::{current_occurrence, index_of, override_of, rule};
use crate::state::State;
use crate::tasks_v1::{
    Assignee, LocalDateTime, Override, OverrideChange, Progress, SeriesScope, Task, TaskPatch,
    TaskResult, not_possible,
};

/// What a scoped write acts on.
pub(crate) enum Target {
    /// The whole task (or a split-off tail, which is a task of its own).
    Whole(usize),
    /// One occurrence of a series.
    Occurrence(usize, LocalDateTime),
}

impl State {
    /// The index of the occurrence `rid` of the series at row `i`; `Invalid` for a plain task or an unknown or
    /// excluded occurrence, `Unsupported` (with `capability`) for a rule the fake cannot expand.
    pub(crate) fn occurrence_index(
        &self,
        i: usize,
        rid: &LocalDateTime,
        capability: &str,
    ) -> TaskResult<u32> {
        let t = &self.rows[i].task;
        if t.recurrence.is_empty() {
            return Err(Self::bad("scope", "not-a-series"));
        }
        if rule(t).is_none() {
            return not_possible(capability);
        }
        let k = index_of(t, rid).ok_or_else(|| Self::bad("recurrence_id", "unknown-occurrence"))?;
        if matches!(
            override_of(t, rid),
            Some(Override {
                change: OverrideChange::Excluded,
                ..
            })
        ) {
            return Err(Self::bad("recurrence_id", "excluded"));
        }
        Ok(k)
    }

    /// Resolve a scope against the task at row `i`.
    pub(crate) fn target(
        &mut self,
        i: usize,
        scope: SeriesScope,
        capability: &str,
    ) -> TaskResult<Target> {
        match scope {
            SeriesScope::Whole => Ok(Target::Whole(i)),
            SeriesScope::This { recurrence_id } => {
                self.occurrence_index(i, &recurrence_id, capability)?;
                Ok(Target::Occurrence(i, recurrence_id))
            }
            SeriesScope::ThisAndFollowing { recurrence_id } => {
                match self.occurrence_index(i, &recurrence_id, capability)? {
                    0 => Ok(Target::Whole(i)),
                    _ => self.split(i, &recurrence_id).map(Target::Whole),
                }
            }
        }
    }

    /// Run `f`; if it fails, put the state back as it was (a split must not survive a failed write).
    pub(crate) fn atomic<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> TaskResult<T>,
    ) -> TaskResult<T> {
        let backup = self.clone();
        let out = f(self);
        if out.is_err() {
            *self = backup;
        }
        out
    }
}

/// The patch, the progress and the assignees of one exception.
pub(crate) type Changed<'a> = (
    &'a mut TaskPatch,
    &'a mut Option<Progress>,
    &'a mut Option<Vec<Assignee>>,
);

/// The values an occurrence has changed (created empty on first use); an excluded occurrence is `Invalid`.
pub(crate) fn changed<'a>(t: &'a mut Task, rid: &LocalDateTime) -> TaskResult<Changed<'a>> {
    let at = t
        .overrides
        .iter()
        .position(|o| o.recurrence_id == *rid)
        .unwrap_or_else(|| {
            t.overrides.push(Override {
                recurrence_id: *rid,
                change: OverrideChange::Changed {
                    patch: TaskPatch::default(),
                    progress: None,
                    assignees: None,
                },
            });
            t.overrides.len() - 1
        });
    match &mut t.overrides[at].change {
        OverrideChange::Changed {
            patch,
            progress,
            assignees,
        } => Ok((patch, progress, assignees)),
        OverrideChange::Excluded => Err(State::bad("recurrence_id", "excluded")),
    }
}

/// Whether the series has no open occurrence left.
pub(crate) fn series_done(t: &Task) -> bool {
    current_occurrence(t).is_none()
}
