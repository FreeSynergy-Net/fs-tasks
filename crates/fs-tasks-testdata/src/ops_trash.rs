//! The trash of a task: delete (whole, one occurrence, this and the following) and restore.
//!
//! Pattern: Memento (a partial delete of a series keeps a `Cut` with what it took away, so `restore` with the same
//! scope gives it back; `This` removes the exception, `ThisAndFollowing` gives back rules and exceptions, but only
//! if nobody changed the series since, else `Changed`).

use crate::clock::{local, naive};
use crate::mutation::Mutation;
use crate::ops_repeat::is_completion;
use crate::series::{base, rule};
use crate::state::{Cut, State};
use crate::tasks_v1::{
    Deleted, Error, HistoryKind, LocalDateTime, ObjectId, Override, OverrideChange, Recurrence,
    RecurrenceRule, Revision, RuleEnd, SUBTASK_DEPTH_MAX, SeriesScope, Task, TaskResult, Utc,
};
use chrono::Duration;

impl State {
    pub(crate) fn delete(
        &mut self,
        id: ObjectId,
        expected: Revision,
        scope: SeriesScope,
    ) -> TaskResult<Deleted> {
        self.expire();
        let i = self.live_expected(id, expected)?;
        self.need_write(self.rows[i].task.list, "task.delete")?;
        if is_completion(&self.rows[i].task) {
            return self.atomic(|s| s.delete_completion(i, scope));
        }
        self.atomic(|s| match scope {
            SeriesScope::Whole => Ok(s.delete_whole(i)),
            SeriesScope::This { recurrence_id } => {
                s.occurrence_index(i, &recurrence_id, "task.delete")?;
                Ok(s.delete_occurrence(i, recurrence_id))
            }
            SeriesScope::ThisAndFollowing { recurrence_id } => {
                match s.occurrence_index(i, &recurrence_id, "task.delete")? {
                    0 => Ok(s.delete_whole(i)),
                    _ => s.cut_following(i, recurrence_id),
                }
            }
        })
    }

    /// A series from completion: `This` at the current occurrence skips it (the last one deletes the task), a later
    /// or earlier one is `Invalid`, `ThisAndFollowing` at the current one is `Whole`.
    fn delete_completion(&mut self, i: usize, scope: SeriesScope) -> TaskResult<Deleted> {
        let current = base(&self.rows[i].task).and_then(local);
        match scope {
            SeriesScope::Whole => Ok(self.delete_whole(i)),
            SeriesScope::This { recurrence_id } if Some(recurrence_id) == current => {
                let last = matches!(
                    self.rows[i].task.recurrence.first(),
                    Some(Recurrence::Rule(r)) if matches!(r.end, RuleEnd::Count(c) if c <= 1)
                );
                if last {
                    Ok(self.delete_whole(i))
                } else {
                    self.skip_completion(i, recurrence_id)
                }
            }
            SeriesScope::ThisAndFollowing { recurrence_id } if Some(recurrence_id) == current => {
                Ok(self.delete_whole(i))
            }
            _ => Err(Self::bad("recurrence_id", "not-current")),
        }
    }

    fn delete_whole(&mut self, i: usize) -> Deleted {
        let id = self.rows[i].task.id;
        let kids = self.descendants(id);
        self.tick();
        let bin = self.new_bin();
        for &k in &kids {
            self.rows[k].deleted = Some(bin);
            self.rows[k].deleted_with = Some(id);
            let (t, l) = (self.rows[k].task.id, self.rows[k].task.list);
            self.feed(t, l, HistoryKind::Delete);
        }
        self.bump(i, HistoryKind::Delete, Vec::new());
        self.rows[i].deleted = Some(bin);
        Deleted {
            id,
            scope: SeriesScope::Whole,
            with_subtasks: kids.into_iter().map(|k| self.rows[k].task.id).collect(),
            restorable: Some(self.bin_state(id, bin)),
        }
    }

    fn delete_occurrence(&mut self, i: usize, rid: LocalDateTime) -> Deleted {
        let bin = self.new_bin();
        let until = bin
            .until
            .unwrap_or(Utc::from_millis(self.now.millis() + 30 * 86_400_000));
        let id = self.rows[i].task.id;
        let t = &mut self.rows[i].task;
        t.claim = None;
        t.checklist.iter_mut().for_each(|c| c.done = None);
        let replaced: Vec<Override> = t
            .overrides
            .iter()
            .filter(|o| o.recurrence_id == rid)
            .cloned()
            .collect();
        t.overrides.retain(|o| o.recurrence_id != rid);
        t.overrides.push(Override {
            recurrence_id: rid,
            change: OverrideChange::Excluded,
        });
        self.bump(i, HistoryKind::Delete, Vec::new());
        let rev_after = self.rows[i].task.revision;
        self.cuts.push(Cut {
            id,
            recurrence_id: rid,
            following: false,
            until,
            rev_after,
            recurrence: Vec::new(),
            overrides: replaced,
        });
        Deleted {
            id,
            scope: SeriesScope::This { recurrence_id: rid },
            with_subtasks: Vec::new(),
            restorable: Some(self.bin_state(id, bin)),
        }
    }

    /// Shorten the series: the rule ends before `rid` (`RuleEnd::Until`), later exceptions go.
    fn cut_following(&mut self, i: usize, rid: LocalDateTime) -> TaskResult<Deleted> {
        let bin = self.new_bin();
        let until = bin
            .until
            .unwrap_or(Utc::from_millis(self.now.millis() + 30 * 86_400_000));
        let old: Task = self.rows[i].task.clone();
        let (Some(r), Some(at)) = (rule(&old), naive(&rid)) else {
            return Err(Self::bad("recurrence_id", "unknown-occurrence"));
        };
        let before =
            local(at - Duration::seconds(1)).ok_or_else(|| Self::bad("recurrence_id", "range"))?;
        let head = RecurrenceRule {
            end: RuleEnd::Until(before),
            ..r.clone()
        };
        let t = &mut self.rows[i].task;
        t.recurrence = vec![Recurrence::Rule(head)];
        t.overrides
            .retain(|o| naive(&o.recurrence_id).is_some_and(|x| x < at));
        self.bump(i, HistoryKind::Delete, Vec::new());
        let rev_after = self.rows[i].task.revision;
        self.cuts.push(Cut {
            id: old.id,
            recurrence_id: rid,
            following: true,
            until,
            rev_after,
            recurrence: old.recurrence,
            overrides: old.overrides,
        });
        Ok(Deleted {
            id: old.id,
            scope: SeriesScope::ThisAndFollowing { recurrence_id: rid },
            with_subtasks: Vec::new(),
            restorable: Some(self.bin_state(old.id, bin)),
        })
    }

    pub(crate) fn restore(&mut self, id: ObjectId, scope: SeriesScope) -> TaskResult<Task> {
        self.expire();
        match scope {
            SeriesScope::Whole => self.restore_whole(id),
            SeriesScope::This { recurrence_id } => self.restore_cut(id, recurrence_id, false),
            SeriesScope::ThisAndFollowing { recurrence_id } => {
                self.restore_cut(id, recurrence_id, true)
            }
        }
    }

    fn restore_whole(&mut self, id: ObjectId) -> TaskResult<Task> {
        // NotFound (also for what the asker may not see), then Forbidden, then Invalid: nothing is given away.
        let i = self
            .at(id)
            .filter(|&i| self.sees_task(i))
            .ok_or(Error::NotFound(id))?;
        let invalid_first = self.mutated(Mutation::RestoreInvalidFirst);
        if !invalid_first {
            self.need_write(self.rows[i].task.list, "task.restore")?;
        }
        if self.rows[i].deleted.is_none() {
            return Err(Self::bad(
                "task",
                if self.trash_of(i).is_some() {
                    "list-in-trash"
                } else {
                    "not-deleted"
                },
            ));
        }
        if self.rows[i].deleted_with.is_some() {
            return Err(Self::bad("task", "deleted-with-parent"));
        }
        if self.list_bins.contains_key(&self.rows[i].task.list) {
            return Err(Self::bad("task", "list-in-trash"));
        }
        if let Some(p) = self.rows[i].task.parent
            && self.at(p).is_some_and(|k| self.trash_of(k).is_some())
        {
            return Err(Self::bad("parent", "in-trash"));
        }
        if self.levels_after_restore(i) > SUBTASK_DEPTH_MAX
            && !self.mutated(Mutation::RestoreNoDepth)
        {
            return Err(Self::bad("parent", "too-deep"));
        }
        if invalid_first {
            self.need_write(self.rows[i].task.list, "task.restore")?;
        }
        for k in 0..self.rows.len() {
            if self.rows[k].deleted_with == Some(id) {
                self.rows[k].deleted = None;
                self.rows[k].deleted_with = None;
                let (t, l) = (self.rows[k].task.id, self.rows[k].task.list);
                self.feed(t, l, HistoryKind::Undo);
            }
        }
        self.rows[i].deleted = None;
        self.bump(i, HistoryKind::Undo, Vec::new());
        Ok(self.rows[i].task.clone())
    }

    /// The levels of the tree once the task at row `i` is back with the subtasks that were deleted with it (its
    /// parent may have moved deeper meanwhile).
    fn levels_after_restore(&self, i: usize) -> usize {
        let t = &self.rows[i].task;
        let above = t.parent.map_or(0, |p| self.depth_of(p));
        let below = self
            .rows
            .iter()
            .filter(|r| r.deleted_with == Some(t.id))
            .map(|r| self.depth_of(r.task.id) - self.depth_of(t.id))
            .max()
            .unwrap_or(0);
        above + 1 + below
    }

    fn restore_cut(
        &mut self,
        id: ObjectId,
        rid: LocalDateTime,
        following: bool,
    ) -> TaskResult<Task> {
        let i = self.live(id)?;
        if is_completion(&self.rows[i].task) && !following {
            return self.undo_completion(i, &rid, true);
        }
        let at = self
            .cuts
            .iter()
            .position(|c| c.id == id && c.recurrence_id == rid && c.following == following)
            .ok_or(Error::NotFound(id))?;
        if following && self.rows[i].task.revision != self.cuts[at].rev_after {
            return Err(Error::Changed {
                current: self.rows[i].task.revision,
            }
            .into());
        }
        let cut = self.cuts.remove(at);
        let t = &mut self.rows[i].task;
        if following {
            t.recurrence = cut.recurrence;
            t.overrides = cut.overrides;
        } else {
            t.overrides.retain(|o| o.recurrence_id != rid);
            t.overrides.extend(cut.overrides);
        }
        self.bump(i, HistoryKind::Undo, Vec::new());
        Ok(self.rows[i].task.clone())
    }
}
