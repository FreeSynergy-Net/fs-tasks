//! Progress (the board columns), order (`place`) and the claim.
//!
//! Pattern: Command methods of the state. Dragging a card on the board is two transactions: `progress` puts the
//! card at the top of the new column, `place` then moves it; a rank that runs out is spread in the same transaction.

use crate::patch::diff;
use crate::ready::is_done;
use crate::scope::{Target, changed, series_done};
use crate::state::State;
use crate::tasks_v1::{
    Children, Claim, HistoryKind, LocalDateTime, ObjectId, Progress, Revision, SeriesScope, Task,
    TaskError, TaskResult, Utc,
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
        self.purge();
        let i = self.live_expected(id, expected)?;
        self.atomic(|s| match s.target(i, scope, "task.progress")? {
            Target::Whole(j) => s.progress_whole(j, to, children),
            Target::Occurrence(j, rid) => s.progress_occurrence(j, &rid, to),
        })
    }

    fn set_progress(&mut self, j: usize, to: Progress) -> TaskResult<()> {
        let old = self.rows[j].task.clone();
        let mut t = old.clone();
        t.progress = to;
        t.progress_updated = Some(self.now);
        t.rank = self.top(&t)?;
        let fields = diff(&old, &t);
        self.rows[j].task = t;
        self.bump(j, HistoryKind::Update, fields);
        Ok(())
    }

    fn progress_whole(&mut self, j: usize, to: Progress, children: Children) -> TaskResult<Task> {
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
        self.purge();
        let i = self.live_expected(id, expected)?;
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

    fn held_by_other(&self, i: usize) -> Option<Claim> {
        let c = self.rows[i].task.claim.clone()?;
        (c.until > self.now && c.by != self.asker()).then_some(c)
    }

    pub(crate) fn claim(
        &mut self,
        id: ObjectId,
        expected: Revision,
        until: Utc,
    ) -> TaskResult<Task> {
        self.purge();
        let i = self.live_expected(id, expected)?;
        if until <= self.now {
            return Err(Self::bad("until", "past"));
        }
        if let Some(c) = self.held_by_other(i) {
            return Err(TaskError::Claimed(c));
        }
        self.rows[i].task.claim = Some(Claim {
            by: self.asker(),
            until,
        });
        self.bump(i, HistoryKind::Join, Vec::new());
        Ok(self.rows[i].task.clone())
    }

    pub(crate) fn release(&mut self, id: ObjectId, expected: Revision) -> TaskResult<Task> {
        self.purge();
        let i = self.live_expected(id, expected)?;
        if let Some(c) = self.held_by_other(i) {
            return Err(TaskError::Claimed(c));
        }
        if self.rows[i].task.claim.take().is_some() {
            self.bump(i, HistoryKind::Leave, Vec::new());
        }
        Ok(self.rows[i].task.clone())
    }
}
