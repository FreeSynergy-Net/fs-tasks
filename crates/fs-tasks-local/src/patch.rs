//! The checks and the application of a `TaskPatch` for the fields this run stores.
//!
//! Pattern: Value object checks (every rule of the trait that says `Invalid` is one check here; `reason` is a code).
//! A field the store does not hold yet (alerts, links, rules ...) is `Unsupported`, named: never dropped silently.

use fs_tasks_testdata::tasks_v1::{RepeatFrom, Task, TaskPatch, TaskResult};

use crate::error::{bad, unsupported};
use crate::zones::check_due;

/// The checks that need no stored state, and the refusal of what this run does not store.
pub fn check(p: &TaskPatch) -> TaskResult<()> {
    if p.title.as_deref().is_some_and(|t| t.trim().is_empty()) {
        return Err(bad("title", "empty"));
    }
    if p.priority.is_some_and(|v| v > 9) {
        return Err(bad("priority", "range"));
    }
    if p.percent_complete.flatten().is_some_and(|v| v > 100) {
        return Err(bad("percent_complete", "range"));
    }
    if let Some(Some(d)) = &p.due {
        check_due(d, "due.zone")?;
    }
    if let Some(Some(d)) = &p.start {
        check_due(d, "start.zone")?;
    }
    if p.labels
        .as_ref()
        .is_some_and(|l| l.iter().any(|x| x.trim().is_empty()))
    {
        return Err(bad("labels", "empty"));
    }
    refuse_unstored(p)
}

/// Fields of run 2: alerts, links and series (rules, exclusions, repeat from completion).
fn refuse_unstored(p: &TaskPatch) -> TaskResult<()> {
    let later = |name: &str| unsupported("task.edit", &format!("{name} is stored in K1 run 2"));
    if p.alerts.is_some() {
        return Err(later("alerts"));
    }
    if p.links.is_some() {
        return Err(later("links"));
    }
    if p.recurrence.as_ref().is_some_and(|r| !r.is_empty())
        || p.excluded_recurrence
            .as_ref()
            .is_some_and(|r| !r.is_empty())
        || p.repeat_from.is_some_and(|r| r != RepeatFrom::Schedule)
    {
        return Err(later("series"));
    }
    Ok(())
}

/// Set the fields a patch names (not `list`, not `parent`: they move the task and are the caller's). Labels are a set
/// (sorted, no duplicates).
pub fn apply(t: &mut Task, p: TaskPatch) {
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
        color
    );
    if let Some(mut l) = p.labels {
        l.sort();
        l.dedup();
        t.labels = l;
    }
}
