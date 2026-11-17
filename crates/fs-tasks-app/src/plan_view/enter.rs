//! The entry line of the plan screens: make a task from what was typed, with what the view gives it.
//!
//! Pattern: Command — one function for both screens; it returns what the screen must tell (`Notice`), never
//! swallows a refusal, and reports a task the view does not show.

use fs_components_workbenches::SourceError;
use fs_components_workbenches::tasks::{TaskDraft, TaskSource};

use super::defaults::{View, defaults_for, fits, landing};
use super::notice::Notice;
use crate::source::TaskAdapter;
use fs_tasks_core::Day;

/// What the store refused, as a message of the screen.
pub(super) fn failed(title: &str, e: &SourceError) -> Notice {
    Notice::CreateFailed {
        title: title.to_string(),
        kind: format!("{:?}", e.kind),
        detail: e.message.clone(),
    }
}

/// The message for a refused move or tick.
pub(super) fn change_failed(e: &SourceError) -> Notice {
    Notice::ChangeFailed {
        kind: format!("{:?}", e.kind),
        detail: e.message.clone(),
    }
}

fn fill(draft: &mut TaskDraft, text: &str, view: &View<'_>, today: Day) {
    let d = defaults_for(view, today);
    for f in &mut draft.fields {
        match f.key.as_str() {
            "title" => f.value = text.to_string(),
            "progress" => {
                if let Some(p) = &d.progress {
                    f.value.clone_from(p);
                }
            }
            "due" => {
                if let Some(day) = d.due {
                    f.value = format!("{} 17:00", day.text());
                }
            }
            "labels" => f.value = d.labels.join(", "),
            _ => {}
        }
    }
}

/// Make the task `text` in `view`.
///
/// # Errors
/// The refusal of the store (or of the blank form) as a [`Notice`]; nothing was made.
pub(super) fn create_task(
    adapter: &TaskAdapter,
    view: &View<'_>,
    today: Day,
    text: &str,
) -> Result<Option<Notice>, Notice> {
    let mut draft = adapter.blank().map_err(|e| failed(text, &e))?;
    fill(&mut draft, text, view, today);
    adapter.create(&draft).map_err(|e| failed(text, &e))?;
    let d = defaults_for(view, today);
    Ok((!fits(view, &d, today)).then(|| Notice::OutsideView {
        title: text.to_string(),
        place: landing(&d, today),
    }))
}
