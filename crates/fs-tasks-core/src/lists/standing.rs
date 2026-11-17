//! How a task stands, and which list holds it.
//!
//! Pattern: Specification — small pure functions of a task, its readiness and today; every view uses the same
//! rules, so a task is "blocked" the same way on a card and in a list. Moved from `fs-tasks-app`.

use crate::day::Day;
use crate::model::Task;
use crate::plan::Readiness;

/// Where a task stands for the person who asks "what next?".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standing {
    /// Open, nothing keeps it from starting, and it is due today or was due before.
    Today,
    /// Open and nothing keeps it from starting.
    Ready,
    /// Open and something keeps it from starting (waiting, a stopped task behind it, an unknown task, a cycle).
    Blocked,
    /// Completed, cancelled or failed.
    Finished,
}

/// How a due date stands against today.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Urgency {
    /// Before today.
    Overdue,
    /// Today.
    Today,
    /// Later.
    Later,
}

/// The date part of the due text, `YYYY-MM-DD`.
fn due_day(task: &Task) -> Option<&str> {
    task.due.as_deref().and_then(|d| d.get(..10))
}

/// How the due date stands, if there is one.
#[must_use]
pub fn urgency(task: &Task, today: Day) -> Option<Urgency> {
    let day = due_day(task)?;
    let today = today.text();
    Some(match day.cmp(today.as_str()) {
        std::cmp::Ordering::Less => Urgency::Overdue,
        std::cmp::Ordering::Equal => Urgency::Today,
        std::cmp::Ordering::Greater => Urgency::Later,
    })
}

/// Is it still to be done?
#[must_use]
pub fn is_open(task: &Task) -> bool {
    matches!(task.effective_progress(), "needs-action" | "in-process")
}

/// The labels in a stable order: those switched on.
#[must_use]
pub fn label_list(task: &Task) -> Vec<String> {
    task.keywords
        .iter()
        .filter(|(_, on)| **on)
        .map(|(k, _)| k.clone())
        .collect()
}

/// Who holds the task: the names of its participants, in order.
#[must_use]
pub fn people_of(task: &Task) -> Vec<String> {
    task.participants
        .values()
        .filter_map(|p| p.name.clone().filter(|n| !n.is_empty()))
        .collect()
}

/// The group of a task, from its readiness (`None`: the picture does not know it yet) and today.
#[must_use]
pub fn standing_of(task: &Task, readiness: Option<&Readiness>, today: Day) -> Standing {
    if !is_open(task) {
        return Standing::Finished;
    }
    match readiness {
        None | Some(Readiness::Ready) => {
            if matches!(
                urgency(task, today),
                Some(Urgency::Overdue | Urgency::Today)
            ) {
                Standing::Today
            } else {
                Standing::Ready
            }
        }
        Some(Readiness::Done) => Standing::Finished,
        Some(_) => Standing::Blocked,
    }
}

/// Open, nothing sorted yet: no due date, no label, nobody holding it.
#[must_use]
pub fn is_inbox(task: &Task, st: Standing) -> bool {
    st != Standing::Finished
        && task.due.is_none()
        && label_list(task).is_empty()
        && people_of(task).is_empty()
}

/// Does the list `list` (`inbox`, `today`, `upcoming`, `anytime`, `someday`, `logbook` or `label:<name>`) hold the
/// task, given its standing and how its due date stands?
#[must_use]
pub fn in_list(task: &Task, st: Standing, due: Option<Urgency>, list: &str) -> bool {
    match list {
        "inbox" => is_inbox(task, st),
        "today" => st == Standing::Today,
        "upcoming" => st == Standing::Ready && due == Some(Urgency::Later),
        "anytime" => st == Standing::Ready && due.is_none(),
        "someday" => st == Standing::Blocked,
        "logbook" => st == Standing::Finished,
        other => other
            .strip_prefix("label:")
            .is_some_and(|l| label_list(task).iter().any(|x| x == l) && st != Standing::Finished),
    }
}
