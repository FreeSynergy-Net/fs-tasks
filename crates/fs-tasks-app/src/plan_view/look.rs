//! How a task stands, and the words for it: which group it belongs to, why it waits, when it is due.
//!
//! Pattern: Specification — small pure functions of a task, its readiness and today; both views use the same
//! rules, so a task is "blocked" in the same words on a card and in a list.

use std::fmt::Write;

pub use fs_tasks_core::lists::{Standing, Urgency, urgency};
use fs_tasks_core::lists::{people_of, standing_of};
use fs_tasks_core::plan::Readiness;
use fs_tasks_core::{Day, Task, TaskEntry};

use super::snapshot::Snapshot;

/// The group of `entry` (the rule lives in `fs-tasks-core`, `lists::standing_of`).
#[must_use]
pub fn standing(snapshot: &Snapshot, entry: &TaskEntry) -> Standing {
    standing_of(
        &entry.task,
        snapshot.readiness(&entry.id.to_string()),
        snapshot.today,
    )
}

/// At most three names, then "and N more".
fn names(snapshot: &Snapshot, uids: &[String]) -> String {
    let shown: Vec<&str> = uids.iter().take(3).map(|u| snapshot.name_of(u)).collect();
    let mut text = shown.join(", ");
    if uids.len() > 3 {
        let _ = write!(text, " and {} more", uids.len() - 3);
    }
    text
}

/// The short marking and the sentence that say why a task cannot start; `None` if nothing keeps it.
#[must_use]
pub fn why(snapshot: &Snapshot, readiness: &Readiness) -> Option<(&'static str, String)> {
    match readiness {
        Readiness::Ready | Readiness::Done => None,
        Readiness::Blocked(uids) => {
            Some(("BLOCKED", format!("waits for {}", names(snapshot, uids))))
        }
        Readiness::DependencyCancelled(uids) => Some((
            "BLOCKED: STOPPED",
            format!("waits for a stopped task: {}", names(snapshot, uids)),
        )),
        Readiness::Unknown(uids) => Some((
            "BLOCKED: UNKNOWN",
            format!(
                "waits for a task that is not here: {}",
                names(snapshot, uids)
            ),
        )),
        Readiness::Conflict(uids) => Some((
            "CYCLE",
            format!("is in a cycle with {}", names(snapshot, uids)),
        )),
    }
}

/// The due date as a phrase: "today 17:00", "overdue, 2026-11-10", "2026-11-20 17:00"; empty if none.
#[must_use]
pub fn due_phrase(task: &Task, today: Day) -> String {
    let Some(due) = task.due.as_deref() else {
        return String::new();
    };
    let time = due.get(11..16).unwrap_or("");
    match urgency(task, today) {
        Some(Urgency::Today) => format!("today {time}").trim_end().to_string(),
        Some(Urgency::Overdue) => format!("overdue, {}", &due[..10]),
        _ => format!("{} {time}", &due[..10]).trim_end().to_string(),
    }
}

/// Who holds the task: the names of its participants, in order (the rule lives in `fs-tasks-core`).
#[must_use]
pub fn people(task: &Task) -> Vec<String> {
    people_of(task)
}
