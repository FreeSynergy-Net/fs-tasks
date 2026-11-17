//! Checks and application of a [`TaskPatch`], and the typed log lines of a change.
//!
//! Pattern: Value object checks (every rule of the trait that says `Invalid` is one check here; `reason` is a code).

use crate::clock::tz;
use crate::ids::invalid;
use crate::series::{base, supported};
use crate::tasks_v1::{
    Due, FieldChange, Frequency, Progress, Recurrence, RepeatFrom, RuleEnd, Skip, Task, TaskError,
    TaskPatch, TaskResult, Value,
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
    keys_once(p.alerts.iter().flatten().map(|a| a.key.as_str()), "alerts")?;
    keys_once(p.links.iter().flatten().map(|l| l.key.as_str()), "links")
}

/// A map by key: no key empty, none twice.
fn keys_once<'a>(keys: impl Iterator<Item = &'a str>, field: &str) -> TaskResult<()> {
    let mut seen = std::collections::BTreeSet::new();
    for k in keys {
        if k.is_empty() || !seen.insert(k) {
            return Err(invalid(field, "key"));
        }
    }
    Ok(())
}

/// The check after the patch: a series needs a reference point; the rule is one the fake reads; a series from
/// completion keeps to its form (`RepeatFrom::Completion`: one rule with frequency, interval and end only, no
/// exclusions, only daily, weekly, monthly or yearly, never `Count(0)`).
pub(crate) fn check_task(t: &Task) -> TaskResult<()> {
    if !t.recurrence.is_empty() && base(t).is_none() {
        return Err(invalid("recurrence", "needs-start-or-due"));
    }
    let completion = t.repeat_from == RepeatFrom::Completion && !t.recurrence.is_empty();
    for r in t.recurrence.iter().chain(&t.excluded_recurrence) {
        if let Recurrence::Rule(rule) = r {
            if completion
                && !matches!(
                    rule.frequency,
                    Frequency::Daily | Frequency::Weekly | Frequency::Monthly | Frequency::Yearly
                )
            {
                return Err(invalid("recurrence.frequency", "completion-frequency"));
            }
            if !supported(rule) {
                return Err(invalid("recurrence", "outside-subset"));
            }
        }
    }
    if completion {
        let [Recurrence::Rule(rule)] = t.recurrence.as_slice() else {
            return Err(invalid("recurrence", "completion-form"));
        };
        if !t.excluded_recurrence.is_empty() || rule.skip != Skip::Omit || rule.rscale.is_some() {
            return Err(invalid("recurrence", "completion-form"));
        }
        if rule.end == RuleEnd::Count(0) {
            return Err(invalid("recurrence.end", "count-zero"));
        }
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
        repeat_from,
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
