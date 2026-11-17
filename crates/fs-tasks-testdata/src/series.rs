//! Series of the fake: occurrences of a plain rule, the current occurrence, exceptions, and the split of a series.
//!
//! Pattern: Strategy (a small rule engine; the real one is shared in fs-jscalendar/fs-calendar, step E). The fake
//! expands only daily, weekly, monthly and yearly rules with an interval and an end (`Never`, `Count`, `Until`); a
//! rule with `by_*` parts or another frequency is `Invalid`. A monthly rule clamps the day to the month's end.

use crate::clock::{local, naive, shift, wall};
use crate::state::State;
use crate::tasks_v1::{
    Assignee, Due, Frequency, HistoryKind, LocalDateTime, Override, OverrideChange, Progress,
    Recurrence, RecurrenceRule, Related, Relation, RuleEnd, Task, TaskResult,
};
use chrono::{Duration, Months, NaiveDateTime};

/// The reference point of a series: `start`, else `due`.
pub(crate) fn base(task: &Task) -> Option<NaiveDateTime> {
    task.start.as_ref().or(task.due.as_ref()).and_then(wall)
}

/// The rule of a series, if it is one the fake can expand.
pub(crate) fn rule(task: &Task) -> Option<&RecurrenceRule> {
    match task.recurrence.first() {
        Some(Recurrence::Rule(r)) => Some(r),
        _ => None,
    }
}

/// Whether a rule is inside the subset the fake expands.
pub(crate) fn supported(r: &RecurrenceRule) -> bool {
    matches!(
        r.frequency,
        Frequency::Daily | Frequency::Weekly | Frequency::Monthly | Frequency::Yearly
    ) && r.interval >= 1
        && r.by_day.is_empty()
        && r.by_month_day.is_empty()
        && r.by_month.is_empty()
        && r.by_year_day.is_empty()
        && r.by_week_no.is_empty()
        && r.by_hour.is_empty()
        && r.by_minute.is_empty()
        && r.by_second.is_empty()
        && r.by_set_position.is_empty()
}

/// The `i`-th occurrence (0 = the base itself); `None` after the end of the rule.
pub(crate) fn nth(r: &RecurrenceRule, base: NaiveDateTime, i: u32) -> Option<NaiveDateTime> {
    let step = i64::from(i) * i64::from(r.interval.max(1));
    let at = match r.frequency {
        Frequency::Daily => base.checked_add_signed(Duration::days(step))?,
        Frequency::Weekly => base.checked_add_signed(Duration::weeks(step))?,
        Frequency::Monthly => base.checked_add_months(Months::new(u32::try_from(step).ok()?))?,
        Frequency::Yearly => {
            base.checked_add_months(Months::new(u32::try_from(step * 12).ok()?))?
        }
        _ => return None,
    };
    match r.end {
        RuleEnd::Never => Some(at),
        RuleEnd::Count(c) => (i < c).then_some(at),
        RuleEnd::Until(u) => (at <= naive(&u)?).then_some(at),
    }
}

/// The index of the occurrence named by `rid` (its original local reference point).
pub(crate) fn index_of(task: &Task, rid: &LocalDateTime) -> Option<u32> {
    let (r, b, target) = (rule(task)?, base(task)?, naive(rid)?);
    let mut i = 0;
    loop {
        let at = nth(r, b, i)?;
        if at == target {
            return Some(i);
        }
        if at > target {
            return None;
        }
        i += 1;
    }
}

/// The override of an occurrence.
pub(crate) fn override_of<'a>(task: &'a Task, rid: &LocalDateTime) -> Option<&'a Override> {
    task.overrides.iter().find(|o| o.recurrence_id == *rid)
}

fn finished(p: Progress) -> bool {
    matches!(
        p,
        Progress::Completed | Progress::Failed | Progress::Cancelled
    )
}

/// The current occurrence: the first one that is neither excluded nor finished.
pub(crate) fn current_occurrence(task: &Task) -> Option<(u32, NaiveDateTime)> {
    let (r, b) = (rule(task)?, base(task)?);
    if crate::ops_repeat::is_completion(task) {
        // From completion the reference point IS the current occurrence; the exceptions are finished ones before it.
        return nth(r, b, 0).map(|at| (0, at));
    }
    let mut i = 0;
    loop {
        let at = nth(r, b, i)?;
        let skip = local(at)
            .and_then(|l| override_of(task, &l))
            .is_some_and(|o| match &o.change {
                OverrideChange::Excluded => true,
                OverrideChange::Changed { progress, .. } => progress.is_some_and(finished),
            });
        if !skip {
            return Some((i, at));
        }
        i += 1;
    }
}

/// What a list or a card shows of a task: for a series the values of its current occurrence.
#[derive(Clone, Debug)]
pub(crate) struct Cur {
    pub occurrence: Option<LocalDateTime>,
    pub due: Option<Due>,
    pub start: Option<Due>,
    pub progress: Progress,
    pub assignees: Vec<Assignee>,
}

/// The values of the current occurrence (a plain task shows its own).
pub(crate) fn current(task: &Task) -> Cur {
    let own = Cur {
        occurrence: None,
        due: task.due.clone(),
        start: task.start.clone(),
        progress: task.progress,
        assignees: task.assignees.clone(),
    };
    let (Some(b), Some((_, at))) = (base(task), current_occurrence(task)) else {
        return own;
    };
    let Some(rid) = local(at) else { return own };
    let mut cur = Cur {
        occurrence: Some(rid),
        due: task.due.as_ref().map(|d| shift(d, b, at)),
        start: task.start.as_ref().map(|d| shift(d, b, at)),
        ..own
    };
    if let Some(Override {
        change:
            OverrideChange::Changed {
                patch,
                progress,
                assignees,
            },
        ..
    }) = override_of(task, &rid)
    {
        if let Some(d) = &patch.due {
            cur.due.clone_from(d);
        }
        if let Some(s) = &patch.start {
            cur.start.clone_from(s);
        }
        if let Some(p) = progress {
            cur.progress = *p;
        }
        if let Some(a) = assignees {
            cur.assignees.clone_from(a);
        }
    }
    cur
}

impl State {
    /// `ThisAndFollowing` at a later occurrence: the series is cut before `rid` and a new task (new `uid`, `First`/
    /// `Next` relations) carries on from there. Returns the index of the new task.
    pub(crate) fn split(&mut self, i: usize, rid: &LocalDateTime) -> TaskResult<usize> {
        let old = self.rows[i].task.clone();
        let k =
            index_of(&old, rid).ok_or_else(|| Self::bad("recurrence_id", "unknown-occurrence"))?;
        let (Some(b), Some(at), Some(r)) = (base(&old), naive(rid), rule(&old)) else {
            return Err(Self::bad("recurrence", "not-a-series"));
        };
        let before =
            local(at - Duration::seconds(1)).ok_or_else(|| Self::bad("recurrence_id", "range"))?;
        let mut fresh = old.clone();
        let mut head = r.clone();
        head.end = RuleEnd::Until(before);
        if let RuleEnd::Count(c) = r.end {
            fresh.recurrence = vec![Recurrence::Rule(RecurrenceRule {
                end: RuleEnd::Count(c - k),
                ..r.clone()
            })];
        }
        let id = self.next_id();
        fresh.id = id;
        fresh.uid = format!("{}-from-{}", old.uid, crate::ids::object_number(id));
        fresh.due = old.due.as_ref().map(|d| shift(d, b, at));
        fresh.start = old.start.as_ref().map(|d| shift(d, b, at));
        fresh
            .overrides
            .retain(|o| naive(&o.recurrence_id).is_some_and(|x| x >= at));
        fresh.related = vec![Related {
            uid: old.uid.clone(),
            relation: Relation::First,
        }];
        fresh.revision = crate::tasks_v1::Revision(1);
        fresh.sequence = 0;
        fresh.created = self.tick();
        fresh.updated = fresh.created;
        let probe = fresh.clone();
        fresh.rank = self.rank_after(&probe, Some(old.id))?;
        let new_uid = fresh.uid.clone();
        self.rows.push(crate::state::Row {
            task: fresh,
            deleted: None,
            deleted_with: None,
        });
        let j = self.rows.len() - 1;
        self.note(j, HistoryKind::Create, Vec::new());
        let master = &mut self.rows[i].task;
        master.recurrence = vec![Recurrence::Rule(head)];
        master
            .overrides
            .retain(|o| naive(&o.recurrence_id).is_some_and(|x| x < at));
        master.related.push(Related {
            uid: new_uid,
            relation: Relation::Next,
        });
        self.bump(i, HistoryKind::Update, Vec::new());
        Ok(j)
    }
}
