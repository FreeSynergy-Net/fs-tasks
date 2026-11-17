//! "Repeat from completion" (`RepeatFrom::Completion`): the SAME task moves on when it is done, skipped or cancelled.
//!
//! Pattern: Command methods of the state. The next reference point is the local day of the completion plus the interval
//! (not the old due date), the time of day stays; the finished occurrence stays as an exception `Changed { progress }`
//! under its old reference point; the last one finishes the task like `Whole`. One call, one transaction, one `Update`.

use crate::clock::{local, naive, shift, tz, wall};
use crate::mutation::Mutation;
use crate::series::base;
use crate::state::State;
use crate::tasks_v1::{
    Children, Deleted, Due, FieldChange, Frequency, HistoryKind, LocalDateTime, Override,
    OverrideChange, Progress, REPEAT_DONE_KEPT, Recurrence, RepeatFrom, RuleEnd, SeriesScope, Task,
    TaskResult, Value,
};
use chrono::{Duration, LocalResult, Months, NaiveDateTime, TimeZone};

/// Whether the task is a series that counts from the completion (the rule is one the fake can read).
pub(crate) fn is_completion(t: &Task) -> bool {
    t.repeat_from == RepeatFrom::Completion
        && matches!(t.recurrence.first(), Some(Recurrence::Rule(_)))
}

/// How a finished occurrence is written down.
#[derive(Clone, Copy)]
pub(crate) enum Outcome {
    Done(Progress),
    Skipped,
}

fn local_field(field: &str, old: NaiveDateTime, new: NaiveDateTime) -> Option<FieldChange> {
    Some(FieldChange {
        field: field.to_owned(),
        row_key: None,
        old: Some(Value::Local(local(old)?)),
        new: Some(Value::Local(local(new)?)),
    })
}

impl State {
    /// The next reference point: the day of the completion in the zone of the task, plus the interval; `None` when the
    /// rule has no occurrence left (`Count(1)`, or the next one lies after `Until`).
    fn next_base(&self, t: &Task, b: NaiveDateTime) -> Option<NaiveDateTime> {
        let Some(Recurrence::Rule(r)) = t.recurrence.first() else {
            return None;
        };
        if matches!(r.end, RuleEnd::Count(c) if c <= 1) && !self.mutated(Mutation::NoLastStop) {
            return None;
        }
        let basis = t.start.as_ref().or(t.due.as_ref());
        let zone = match basis {
            Some(Due::Timed { zone, .. }) => tz(zone).unwrap_or(self.zone),
            _ => self.zone,
        };
        let today = if self.mutated(Mutation::RepeatFromDue) {
            b.date()
        } else {
            chrono::DateTime::from_timestamp_millis(self.now.millis())?
                .with_timezone(&zone)
                .date_naive()
        };
        let n = r.interval.max(1);
        let day = match r.frequency {
            Frequency::Daily => today.checked_add_signed(Duration::days(i64::from(n)))?,
            Frequency::Weekly => today.checked_add_signed(Duration::weeks(i64::from(n)))?,
            Frequency::Monthly => today.checked_add_months(Months::new(n))?,
            Frequency::Yearly => today.checked_add_months(Months::new(n.checked_mul(12)?))?,
            _ => return None,
        };
        let mut next = NaiveDateTime::new(day, b.time());
        if matches!(basis, Some(Due::Timed { .. }))
            && matches!(zone.from_local_datetime(&next), LocalResult::None)
        {
            next += Duration::hours(1);
        }
        match r.end {
            RuleEnd::Until(u) if naive(&u).is_none_or(|u| next > u) => None,
            _ => Some(next),
        }
    }

    /// Finish the current occurrence with `outcome` and move on (or finish the task when it was the last one).
    pub(crate) fn advance(&mut self, i: usize, outcome: Outcome) -> TaskResult<()> {
        let old = self.rows[i].task.clone();
        let b = base(&old).ok_or_else(|| Self::bad("recurrence", "needs-start-or-due"))?;
        let rid = local(b).ok_or_else(|| Self::bad("recurrence_id", "range"))?;
        let Some(next) = self.next_base(&old, b) else {
            return self.finish_series(i, outcome);
        };
        let now = self.now;
        let stay = self.mutated(Mutation::TicksStay);
        let mut fields = Vec::new();
        let t = &mut self.rows[i].task;
        for (name, due) in [("start", &mut t.start), ("due", &mut t.due)] {
            if let Some(d) = due.as_mut() {
                let moved = shift(d, b, next);
                if let (Some(from), Some(to)) = (wall(d), wall(&moved)) {
                    fields.extend(local_field(name, from, to));
                }
                *d = moved;
            }
        }
        if let Some(Recurrence::Rule(r)) = t.recurrence.first_mut()
            && let RuleEnd::Count(c) = &mut r.end
        {
            *c = c.saturating_sub(1);
        }
        let change = match outcome {
            Outcome::Done(p) => OverrideChange::Changed {
                patch: crate::tasks_v1::TaskPatch::default(),
                progress: Some(p),
                assignees: None,
            },
            Outcome::Skipped => OverrideChange::Excluded,
        };
        match t.overrides.iter().position(|o| o.recurrence_id == rid) {
            Some(at) => match (&mut t.overrides[at].change, outcome) {
                (OverrideChange::Changed { progress, .. }, Outcome::Done(p)) => *progress = Some(p),
                (slot, _) => *slot = change,
            },
            None => t.overrides.push(Override {
                recurrence_id: rid,
                change,
            }),
        }
        while t.overrides.len() > REPEAT_DONE_KEPT {
            t.overrides.remove(0);
        }
        t.progress = Progress::NeedsAction;
        t.progress_updated = Some(now);
        t.claim = None;
        if !stay {
            t.checklist.iter_mut().for_each(|c| c.done = None);
        }
        self.bump(i, HistoryKind::Update, fields);
        Ok(())
    }

    /// The last occurrence: the task takes the state like `Whole`, the rule stays as it is.
    fn finish_series(&mut self, i: usize, outcome: Outcome) -> TaskResult<()> {
        let to = match outcome {
            Outcome::Done(p) => p,
            Outcome::Skipped => Progress::Cancelled,
        };
        self.set_progress_whole(i, to)
    }

    /// `progress` on a completion series; `Ok(None)` = nothing special, the caller does the plain `Whole` write.
    pub(crate) fn completion_progress(
        &mut self,
        i: usize,
        to: Progress,
        children: Children,
        scope: SeriesScope,
    ) -> TaskResult<Option<Task>> {
        let b = base(&self.rows[i].task);
        let current = b.and_then(local);
        let rid = match scope {
            SeriesScope::Whole => None,
            SeriesScope::This { recurrence_id }
            | SeriesScope::ThisAndFollowing { recurrence_id } => Some(recurrence_id),
        };
        let newest = self.rows[i].task.overrides.last().map(|o| o.recurrence_id);
        if to == Progress::NeedsAction
            && let SeriesScope::This { recurrence_id } = scope
            && newest == Some(recurrence_id)
        {
            // Completing twice on one day leaves the new reference point equal to the finished one: the newest
            // exception is still the one to take back.
            return self.undo_completion(i, &recurrence_id, false).map(Some);
        }
        let old = rid.filter(|r| Some(*r) != current);
        if let Some(r) = old {
            return if to == Progress::NeedsAction && matches!(scope, SeriesScope::This { .. }) {
                self.undo_completion(i, &r, false).map(Some)
            } else {
                Err(Self::bad("recurrence_id", "not-current"))
            };
        }
        let this = matches!(scope, SeriesScope::This { .. });
        let moves = match to {
            Progress::Completed => true,
            Progress::Cancelled | Progress::Failed => this,
            Progress::NeedsAction | Progress::InProcess => false,
        };
        if !moves {
            return Ok(None);
        }
        self.settle_children(i, to, children)?;
        self.advance(i, Outcome::Done(to))?;
        Ok(Some(self.rows[i].task.clone()))
    }

    /// `delete(This)` on the current occurrence of a completion series: skip it (exception `Excluded`).
    pub(crate) fn skip_completion(&mut self, i: usize, rid: LocalDateTime) -> TaskResult<Deleted> {
        let current = base(&self.rows[i].task).and_then(local);
        if current != Some(rid) {
            return Err(Self::bad("recurrence_id", "not-current"));
        }
        let id = self.rows[i].task.id;
        self.advance(i, Outcome::Skipped)?;
        let bin = self.new_bin();
        Ok(Deleted {
            id,
            scope: SeriesScope::This { recurrence_id: rid },
            with_subtasks: Vec::new(),
            restorable: Some(crate::bin::restorable(bin, None, self.now)),
        })
    }

    /// Take back the NEWEST exception: the reference point goes back to `rid` and the exception goes (`Count` + 1). An
    /// older one is `Invalid`. `excluded` says which kind is undone (`restore(This)` or `progress(NeedsAction, This)`).
    pub(crate) fn undo_completion(
        &mut self,
        i: usize,
        rid: &LocalDateTime,
        excluded: bool,
    ) -> TaskResult<Task> {
        let t = &self.rows[i].task;
        let at = t
            .overrides
            .iter()
            .position(|o| o.recurrence_id == *rid)
            .ok_or_else(|| Self::bad("recurrence_id", "unknown-occurrence"))?;
        if at + 1 != t.overrides.len() && !self.mutated(Mutation::UndoAny) {
            return Err(Self::bad("recurrence_id", "not-newest"));
        }
        let is_excluded = matches!(t.overrides[at].change, OverrideChange::Excluded);
        if is_excluded != excluded {
            return Err(Self::bad("recurrence_id", "wrong-kind"));
        }
        let (Some(b), Some(back)) = (base(t), naive(rid)) else {
            return Err(Self::bad("recurrence_id", "range"));
        };
        let t = &mut self.rows[i].task;
        for due in [&mut t.start, &mut t.due].into_iter().flatten() {
            *due = shift(due, b, back);
        }
        if let Some(Recurrence::Rule(r)) = t.recurrence.first_mut()
            && let RuleEnd::Count(c) = &mut r.end
        {
            *c += 1;
        }
        t.overrides.remove(at);
        self.bump(i, HistoryKind::Undo, Vec::new());
        Ok(self.rows[i].task.clone())
    }
}
