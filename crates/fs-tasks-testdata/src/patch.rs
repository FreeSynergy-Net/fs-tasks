//! Checks and application of a [`TaskPatch`], and the typed log lines of a change.
//!
//! Pattern: Value object checks (every rule of the trait that says `Invalid` is one check here; `reason` is a code).

use crate::clock::tz;
use crate::ids::invalid;
use crate::series::{base, supported};
use crate::tasks_v1::{
    Due, FieldChange, Progress, Recurrence, Task, TaskError, TaskPatch, TaskResult, Value,
};

/// A timed due value needs a known zone (never guessed, ADR 0085).
pub(crate) fn check_due(due: &Due) -> TaskResult<()> {
    if let Due::Timed { zone, .. } = due {
        tz(zone)?;
    }
    Ok(())
}

/// The checks that need no stored state.
pub(crate) fn check_patch(p: &TaskPatch) -> TaskResult<()> {
    if p.title.as_deref().is_some_and(|t| t.trim().is_empty()) {
        return Err(invalid("title", "empty"));
    }
    if p.priority.is_some_and(|v| v > 9) {
        return Err(invalid("priority", "range"));
    }
    if p.percent_complete.flatten().is_some_and(|v| v > 100) {
        return Err(invalid("percent_complete", "range"));
    }
    for due in [&p.due, &p.start].into_iter().flatten().flatten() {
        check_due(due)?;
    }
    if p.labels
        .as_ref()
        .is_some_and(|l| l.iter().any(|x| x.trim().is_empty()))
    {
        return Err(invalid("labels", "empty"));
    }
    if p.links
        .as_ref()
        .is_some_and(|l| l.iter().any(|x| x.href.is_some() == x.file.is_some()))
    {
        return Err(invalid("links", "href-or-file"));
    }
    for r in [&p.recurrence, &p.excluded_recurrence]
        .into_iter()
        .flatten()
        .flatten()
    {
        if matches!(r, Recurrence::Rule(rule) if !supported(rule)) {
            return Err(invalid("recurrence", "outside-subset"));
        }
    }
    Ok(())
}

/// The check after the patch: a series needs a reference point.
pub(crate) fn check_task(t: &Task) -> TaskResult<()> {
    if !t.recurrence.is_empty() && base(t).is_none() {
        return Err(invalid("recurrence", "needs-start-or-due"));
    }
    Ok(())
}

/// Set the fields a patch names (not `list`, not `parent`: they move the task and are the caller's). Labels are a
/// set (sorted, no duplicates).
pub(crate) fn apply(t: &mut Task, p: TaskPatch) {
    if let Some(v) = p.title {
        t.title = v;
    }
    if let Some((text, kind)) = p.description {
        t.description = text;
        t.description_type = kind;
    }
    macro_rules! set {
        ($($f:ident),*) => {$( if let Some(v) = p.$f { t.$f = v; } )*};
    }
    set!(
        locale,
        due,
        start,
        estimated,
        percent_complete,
        priority,
        color,
        alerts,
        links,
        recurrence,
        excluded_recurrence
    );
    if let Some(mut l) = p.labels {
        l.sort();
        l.dedup();
        t.labels = l;
    }
}

fn text(field: &str, old: String, new: String) -> FieldChange {
    FieldChange {
        field: field.to_owned(),
        row_key: None,
        old: Some(Value::Text(old)),
        new: Some(Value::Text(new)),
    }
}

fn word(p: Progress) -> &'static str {
    match p {
        Progress::NeedsAction => "needs-action",
        Progress::InProcess => "in-process",
        Progress::Completed => "completed",
        Progress::Failed => "failed",
        Progress::Cancelled => "cancelled",
    }
}

/// The typed log lines of a change (the scalar fields; the rest is read from the tasks themselves).
pub(crate) fn diff(old: &Task, new: &Task) -> Vec<FieldChange> {
    let mut v = Vec::new();
    if old.title != new.title {
        v.push(text("title", old.title.clone(), new.title.clone()));
    }
    if old.progress != new.progress {
        v.push(text(
            "progress",
            word(old.progress).to_owned(),
            word(new.progress).to_owned(),
        ));
    }
    if old.priority != new.priority {
        v.push(FieldChange {
            field: "priority".to_owned(),
            row_key: None,
            old: Some(Value::Int(i64::from(old.priority))),
            new: Some(Value::Int(i64::from(new.priority))),
        });
    }
    if old.rank != new.rank {
        v.push(text(
            "rank",
            old.rank.as_str().to_owned(),
            new.rank.as_str().to_owned(),
        ));
    }
    if old.list != new.list {
        v.push(FieldChange {
            field: "list".to_owned(),
            row_key: None,
            old: Some(Value::Object(old.list)),
            new: Some(Value::Object(new.list)),
        });
    }
    v
}

/// A `Cycle` error from the `uid`s along a cycle.
pub(crate) fn cycle(uids: Vec<String>) -> TaskError {
    TaskError::Cycle(uids)
}
