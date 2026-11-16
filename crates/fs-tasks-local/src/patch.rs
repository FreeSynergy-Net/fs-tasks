//! The checks and the application of a `TaskPatch` for the fields this run stores.
//!
//! Pattern: Value object checks (every rule of the trait that says `Invalid` is one check here; `reason` is a code).
//! A field the store does not hold yet (the series: rules, exclusions, repeat from completion) is `Unsupported`, named:
//! never dropped silently.

use std::collections::BTreeSet;

use fs_tasks_testdata::tasks_v1::{Alert, Link, RepeatFrom, Task, TaskPatch, TaskResult};

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
    if let Some(a) = &p.alerts {
        check_alerts(a)?;
    }
    if let Some(l) = &p.links {
        check_links(l)?;
    }
    refuse_unstored(p)
}

/// Every key is given once and not empty (a map by key); `field` names where the problem is.
fn keys_ok<'a>(keys: impl Iterator<Item = &'a str>, field: &str) -> TaskResult<()> {
    let mut seen = BTreeSet::new();
    for k in keys {
        if k.is_empty() {
            return Err(bad(field, "key-empty"));
        }
        if !seen.insert(k) {
            return Err(bad(field, "key-twice"));
        }
    }
    Ok(())
}

fn check_alerts(alerts: &[Alert]) -> TaskResult<()> {
    keys_ok(alerts.iter().map(|a| a.key.as_str()), "alerts")
}

/// A link is an address or a file of ours (exactly one), and its size fits a signed number.
fn check_links(links: &[Link]) -> TaskResult<()> {
    keys_ok(links.iter().map(|l| l.key.as_str()), "links")?;
    if links.iter().any(|l| l.href.is_some() == l.file.is_some()) {
        return Err(bad("links", "href-or-file"));
    }
    if links
        .iter()
        .any(|l| l.size.is_some_and(|n| i64::try_from(n).is_err()))
    {
        return Err(bad("links", "size"));
    }
    Ok(())
}

/// Fields of the series (rules, exclusions, repeat from completion): they come with run 2a rest 3.
fn refuse_unstored(p: &TaskPatch) -> TaskResult<()> {
    let later = |name: &str| unsupported("task.edit", &format!("{name} is stored in K1 run 2"));
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
        color,
        alerts,
        links
    );
    if let Some(mut l) = p.labels {
        l.sort();
        l.dedup();
        t.labels = l;
    }
}
