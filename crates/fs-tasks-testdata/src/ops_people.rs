//! People on a task: assign, unassign and respond (with the series scope).
//!
//! Pattern: Template method (`change_assignees` does the compare-and-set, the scope and the write; each command is
//! only the change of the list of assignees). `assign` and `unassign` are idempotent: no change, no new revision.

use crate::scope::{Target, changed};
use crate::series::override_of;
use crate::state::{State, check};
use crate::tasks_v1::{
    Answer, Assignee, AssigneeStatus, Error, FieldChange, HistoryKind, ObjectId, OverrideChange,
    PersonRef, Revision, SeriesScope, Task, TaskResult, Value,
};
use fs_types::BasicValue;

type Edit = Option<(HistoryKind, FieldChange)>;

fn person_line(who: &PersonRef, added: bool) -> FieldChange {
    let value = Some(Value::Person(who.clone()));
    let (old, new) = if added { (None, value) } else { (value, None) };
    FieldChange {
        field: "assignees".to_owned(),
        row_key: Some(who.id.canonical()),
        old,
        new,
    }
}

impl State {
    fn change_assignees(
        &mut self,
        id: ObjectId,
        expected: Revision,
        scope: SeriesScope,
        capability: &str,
        edit: impl FnOnce(&mut Vec<Assignee>) -> TaskResult<Edit>,
    ) -> TaskResult<Task> {
        self.expire();
        let i = self.live(id)?;
        // Answering is the right of the assigned person themselves; every other change needs the right to write.
        if capability != "task.respond" {
            self.need_write(self.rows[i].task.list, capability)?;
        }
        check(self.rows[i].task.revision, expected)?;
        self.atomic(|s| {
            let (j, rid) = match s.target(i, scope, capability)? {
                Target::Whole(j) => (j, None),
                Target::Occurrence(j, rid) => (j, Some(rid)),
            };
            let t = &mut s.rows[j].task;
            let mut list = match &rid {
                None => t.assignees.clone(),
                Some(r) => match override_of(t, r) {
                    Some(o) => match &o.change {
                        OverrideChange::Changed {
                            assignees: Some(a), ..
                        } => a.clone(),
                        _ => t.assignees.clone(),
                    },
                    None => t.assignees.clone(),
                },
            };
            let Some((kind, line)) = edit(&mut list)? else {
                return Ok(t.clone());
            };
            match &rid {
                None => t.assignees = list,
                Some(r) => *changed(t, r)?.2 = Some(list),
            }
            s.bump(j, kind, vec![line]);
            Ok(s.rows[j].task.clone())
        })
    }

    pub(crate) fn assign(
        &mut self,
        id: ObjectId,
        expected: Revision,
        who: &PersonRef,
        scope: SeriesScope,
    ) -> TaskResult<Task> {
        let (by, now) = (self.asker(), self.tick());
        self.change_assignees(id, expected, scope, "task.assign", |list| {
            if list.iter().any(|a| a.who == *who) {
                return Ok(None);
            }
            list.push(Assignee {
                who: who.clone(),
                status: AssigneeStatus::NeedsAction,
                assigned_by: Some(by),
                assigned: Some(now),
                comment: None,
            });
            Ok(Some((HistoryKind::Add, person_line(who, true))))
        })
    }

    pub(crate) fn unassign(
        &mut self,
        id: ObjectId,
        expected: Revision,
        who: &PersonRef,
        scope: SeriesScope,
    ) -> TaskResult<Task> {
        self.change_assignees(id, expected, scope, "task.assign", |list| {
            let before = list.len();
            list.retain(|a| a.who != *who);
            Ok((list.len() != before).then(|| (HistoryKind::Remove, person_line(who, false))))
        })
    }

    /// The asking person answers for their own row only; not assigned is `Forbidden`.
    pub(crate) fn respond(
        &mut self,
        id: ObjectId,
        expected: Revision,
        answer: Answer,
        comment: Option<&str>,
        scope: SeriesScope,
    ) -> TaskResult<Task> {
        let me = (self.account.clone(), self.me_card.clone());
        self.change_assignees(id, expected, scope, "task.respond", |list| {
            let row = list
                .iter_mut()
                .find(|a| a.who == me.0 || Some(&a.who) == me.1.as_ref());
            let Some(row) = row else {
                return Err(Error::Forbidden {
                    capability: "task.respond".to_owned(),
                }
                .into());
            };
            let (status, kind) = match answer {
                Answer::Accept => (AssigneeStatus::Accepted, HistoryKind::Accept),
                Answer::Decline => (AssigneeStatus::Declined, HistoryKind::Reject),
            };
            row.status = status;
            row.comment = comment.map(str::to_owned);
            Ok(Some((kind, person_line(&row.who.clone(), true))))
        })
    }
}
