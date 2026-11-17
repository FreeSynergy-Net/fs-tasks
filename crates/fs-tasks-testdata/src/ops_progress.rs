//! Progress (the board columns) and order (`place`); the claim is in `ops_claim.rs`.
//!
//! Pattern: Command methods of the state. Dragging a card on the board is two transactions: `progress` puts the
//! card at the top of the new column, `place` then moves it; a rank that runs out is spread in the same transaction.

use crate::ops_repeat::is_completion;
use crate::patch::diff;
use crate::ready::is_done;
use crate::scope::{Target, changed, series_done};
use crate::state::{State, check};
use crate::tasks_v1::{
    Children, HistoryKind, LocalDateTime, ObjectId, Progress, Revision, SeriesScope, Task,
    TaskError, TaskResult,
};

impl State {
    pub(crate) fn progress(
        &mut self,
        id: ObjectId,
        expected: Revision,
        to: Progress,
        children: Children,
        scope: SeriesScope,
    ) -> TaskResult<Task> {
        self.expire();
        let i = self.live(id)?;
        self.need_write(self.rows[i].task.list, "task.progress")?;
        check(self.rows[i].task.revision, expected)?;
        self.atomic(|s| {
            if is_completion(&s.rows[i].task) {
                return match s.completion_progress(i, to, children, scope)? {
                    Some(done) => Ok(done),
                    None => s.progress_whole(i, to, children),
                };
            }
            match s.target(i, scope, "task.progress")? {
                Target::Whole(j) => s.progress_whole(j, to, children),
                Target::Occurrence(j, rid) => s.progress_occurrence(j, &rid, to),
            }
        })
    }

    fn set_progress(&mut self, j: usize, to: Progress) -> TaskResult<()> {
        let old = self.rows[j].task.clone();
        let mut t = old.clone();
        t.progress = to;
        t.progress_updated = Some(self.now);
        t.rank = self.top(&t)?;
        if is_done(to) {
            t.claim = None;
        }
        let fields = diff(&old, &t);
        self.rows[j].task = t;
        self.bump(j, HistoryKind::Update, fields);
        if is_done(to) && old.claim.is_some() {
            self.note(j, HistoryKind::Leave, Vec::new());
        }
        Ok(())
    }

    /// Set the state of the whole task (a finished state ends the claim: `Leave` in the log).
    pub(crate) fn set_progress_whole(&mut self, j: usize, to: Progress) -> TaskResult<()> {
        self.set_progress(j, to)
    }

    /// What happens to the open subtasks when a task is finished (`Children`).
    pub(crate) fn settle_children(
        &mut self,
        j: usize,
        to: Progress,
        children: Children,
    ) -> TaskResult<()> {
        let id = self.rows[j].task.id;
        let open: Vec<usize> = self
            .descendants(id)
            .into_iter()
            .filter(|&k| !is_done(self.rows[k].task.progress))
            .collect();
        if is_done(to) && !open.is_empty() {
            match children {
                Children::Refuse => {
                    return Err(TaskError::OpenSubtasks(
                        open.iter().map(|&k| self.rows[k].task.id).collect(),
                    ));
                }
                Children::Same => {
                    for k in open {
                        self.set_progress(k, to)?;
                    }
                }
                Children::Leave => {}
            }
        }
        Ok(())
    }

    fn progress_whole(&mut self, j: usize, to: Progress, children: Children) -> TaskResult<Task> {
        self.settle_children(j, to, children)?;
        if self.rows[j].task.progress != to {
            self.set_progress(j, to)?;
        }
        Ok(self.rows[j].task.clone())
    }

    /// One occurrence: it gets its own `progress`; the next one becomes the current one. When none is left the
    /// series is finished.
    fn progress_occurrence(
        &mut self,
        j: usize,
        rid: &LocalDateTime,
        to: Progress,
    ) -> TaskResult<Task> {
        let t = &mut self.rows[j].task;
        *changed(t, rid)?.1 = Some(to);
        if is_done(to) {
            // The series moves on: the ticks of the checklist and the claim end (tasks_v1 `Checklist`, claim rule).
            t.claim = None;
            t.checklist.iter_mut().for_each(|c| c.done = None);
        }
        if series_done(&self.rows[j].task) {
            return self
                .set_progress(j, Progress::Completed)
                .map(|()| self.rows[j].task.clone());
        }
        self.bump(j, HistoryKind::Update, Vec::new());
        Ok(self.rows[j].task.clone())
    }

    pub(crate) fn place(
        &mut self,
        id: ObjectId,
        expected: Revision,
        after: Option<ObjectId>,
    ) -> TaskResult<Task> {
        self.expire();
        let i = self.live_expected(id, expected)?;
        self.need_write(self.rows[i].task.list, "task.place")?;
        if after == Some(id) {
            return Err(Self::bad("after", "itself"));
        }
        let old = self.rows[i].task.clone();
        let mut t = old.clone();
        t.rank = self.rank_after(&old, after)?;
        let fields = diff(&old, &t);
        self.rows[i].task = t;
        self.bump(i, HistoryKind::Move, fields);
        Ok(self.rows[i].task.clone())
    }
}
