//! The trash of a task: delete (whole, one occurrence, this and the following) and restore.
//!
//! Pattern: Memento (a partial delete of a series keeps a `Cut` with what it took away, so `restore` with the same
//! scope gives it back; `This` removes the exception, `ThisAndFollowing` gives back rules and exceptions, but only
//! if nobody changed the series since, else `Changed`).

use crate::clock::{local, naive};
use crate::series::rule;
use crate::state::{Cut, State};
use crate::tasks_v1::{
    Deleted, Error, HistoryKind, LocalDateTime, ObjectId, Override, OverrideChange, Recurrence,
    RecurrenceRule, Revision, RuleEnd, SeriesScope, Task, TaskResult,
};
use chrono::Duration;

impl State {
    pub(crate) fn delete(
        &mut self,
        id: ObjectId,
        expected: Revision,
        scope: SeriesScope,
    ) -> TaskResult<Deleted> {
        self.purge();
        let i = self.live_expected(id, expected)?;
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

    fn delete_whole(&mut self, i: usize) -> Deleted {
        let id = self.rows[i].task.id;
        let kids = self.descendants(id);
        self.tick();
        let until = self.trash_until();
        for &k in &kids {
            self.rows[k].deleted = Some(until);
            self.rows[k].deleted_with = Some(id);
            let (t, l) = (self.rows[k].task.id, self.rows[k].task.list);
            self.feed(t, l, HistoryKind::Delete);
        }
        self.bump(i, HistoryKind::Delete, Vec::new());
        self.rows[i].deleted = Some(until);
        Deleted {
            id,
            scope: SeriesScope::Whole,
            with_subtasks: kids.into_iter().map(|k| self.rows[k].task.id).collect(),
            restorable_until: Some(until),
        }
    }

    fn delete_occurrence(&mut self, i: usize, rid: LocalDateTime) -> Deleted {
        let until = self.trash_until();
        let id = self.rows[i].task.id;
        let t = &mut self.rows[i].task;
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
            restorable_until: Some(until),
        }
    }

    /// Shorten the series: the rule ends before `rid` (`RuleEnd::Until`), later exceptions go.
    fn cut_following(&mut self, i: usize, rid: LocalDateTime) -> TaskResult<Deleted> {
        let until = self.trash_until();
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
            restorable_until: Some(until),
        })
    }

    pub(crate) fn restore(&mut self, id: ObjectId, scope: SeriesScope) -> TaskResult<Task> {
        self.purge();
        match scope {
            SeriesScope::Whole => self.restore_whole(id),
            SeriesScope::This { recurrence_id } => self.restore_cut(id, recurrence_id, false),
            SeriesScope::ThisAndFollowing { recurrence_id } => {
                self.restore_cut(id, recurrence_id, true)
            }
        }
    }

    fn restore_whole(&mut self, id: ObjectId) -> TaskResult<Task> {
        let i = self.at(id).ok_or(Error::NotFound(id))?;
        if self.rows[i].deleted.is_none() {
            return Err(Self::bad(
                "task",
                if self.deleted_until(i).is_some() {
                    "list-in-trash"
                } else {
                    "not-deleted"
                },
            ));
        }
        if self.rows[i].deleted_with.is_some() {
            return Err(Self::bad("task", "deleted-with-parent"));
        }
        if self
            .list_at(self.rows[i].task.list)
            .is_some_and(|l| self.lists[l].restorable_until.is_some())
        {
            return Err(Self::bad("task", "list-in-trash"));
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

    fn restore_cut(
        &mut self,
        id: ObjectId,
        rid: LocalDateTime,
        following: bool,
    ) -> TaskResult<Task> {
        let i = self.live(id)?;
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
