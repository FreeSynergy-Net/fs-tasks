//! What a new task takes over from the view it is typed in.
//!
//! Pattern: Strategy (one rule per view) + Specification (`fits`) — pure functions, so every view can be tested
//! without a window. The entry line of a view must not make a task that the view then hides: the task takes the
//! project, the status or the day of the view; and where the view cannot hold it (the logbook, "someday"), the
//! caller says so (the window shows a notice).

use crate::day::Day;

/// The view a task is typed in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View<'a> {
    /// The board: the project (label) chosen on the left, if any, and the column of the entry line.
    Board {
        /// The label chosen, `None` for all tasks.
        label: Option<&'a str>,
        /// The `progress` value of the column.
        column: &'a str,
    },
    /// The day list: `place` is `inbox`, `today`, `upcoming`, `anytime`, `someday`, `logbook` or `label:<name>`.
    List {
        /// The list shown.
        place: &'a str,
    },
}

/// What a new task has, besides its title.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NewTaskDefaults {
    /// The labels (projects).
    pub labels: Vec<String>,
    /// The `progress`; `None` is a new, open task.
    pub progress: Option<String>,
    /// The day it is due; `None` is no date.
    pub due: Option<Day>,
}

fn finished(d: &NewTaskDefaults) -> bool {
    matches!(
        d.progress.as_deref(),
        Some("completed" | "failed" | "cancelled")
    )
}

/// The defaults of a new task typed in `view`; `today` is the day the lists are cut at.
#[must_use]
pub fn defaults_for(view: &View<'_>, today: Day) -> NewTaskDefaults {
    match *view {
        View::Board { label, column } => NewTaskDefaults {
            labels: label.map(str::to_string).into_iter().collect(),
            progress: Some(column.to_string()),
            due: None,
        },
        View::List { place } => match place {
            "today" => NewTaskDefaults {
                due: Some(today),
                ..NewTaskDefaults::default()
            },
            "upcoming" => NewTaskDefaults {
                due: Some(today.plus(1)),
                ..NewTaskDefaults::default()
            },
            other => NewTaskDefaults {
                labels: other
                    .strip_prefix("label:")
                    .map(str::to_string)
                    .into_iter()
                    .collect(),
                ..NewTaskDefaults::default()
            },
        },
    }
}

/// Is a task with these defaults shown in `view`?
#[must_use]
pub fn fits(view: &View<'_>, d: &NewTaskDefaults, today: Day) -> bool {
    match *view {
        View::Board { label, .. } => label.is_none_or(|l| d.labels.iter().any(|x| x == l)),
        View::List { place } => match place {
            "inbox" => !finished(d) && d.due.is_none() && d.labels.is_empty(),
            "today" => !finished(d) && d.due.is_some_and(|x| x <= today),
            "upcoming" => !finished(d) && d.due.is_some_and(|x| x > today),
            "anytime" => !finished(d) && d.due.is_none(),
            "logbook" => finished(d),
            "someday" => false, // a new task waits for nothing
            other => {
                !finished(d)
                    && other
                        .strip_prefix("label:")
                        .is_some_and(|l| d.labels.iter().any(|x| x == l))
            }
        },
    }
}

/// The list a task with these defaults is found in: its project, else its day, else the inbox.
#[must_use]
pub fn landing(d: &NewTaskDefaults, today: Day) -> String {
    if finished(d) {
        "logbook".into()
    } else if let Some(l) = d.labels.first() {
        format!("label:{l}")
    } else {
        match d.due {
            Some(x) if x <= today => "today".into(),
            Some(_) => "upcoming".into(),
            None => "inbox".into(),
        }
    }
}
