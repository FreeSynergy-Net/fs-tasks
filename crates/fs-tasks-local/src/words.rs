//! The stored words of the enumerations (JSCalendar words) and back.
//!
//! Pattern: Adapter (value to column word and back; an unknown word in a row is `Unavailable`, never a guess).

use fs_objects::v5::codec::damaged;
use fs_tasks_testdata::tasks_v1::{
    AlertAction, DependencyKind, Progress, RelativeTo, TaskResult, TextType,
};

use crate::error::adapt;

/// The stored word of a progress.
#[must_use]
pub fn progress_word(p: Progress) -> &'static str {
    match p {
        Progress::NeedsAction => "needs-action",
        Progress::InProcess => "in-process",
        Progress::Completed => "completed",
        Progress::Failed => "failed",
        Progress::Cancelled => "cancelled",
    }
}

/// The progress of a stored word.
pub fn progress_of(word: &str) -> TaskResult<Progress> {
    Ok(match word {
        "needs-action" => Progress::NeedsAction,
        "in-process" => Progress::InProcess,
        "completed" => Progress::Completed,
        "failed" => Progress::Failed,
        "cancelled" => Progress::Cancelled,
        _ => return Err(adapt(damaged("progress"))),
    })
}

/// The stored word of a text type.
#[must_use]
pub fn text_type_word(t: TextType) -> &'static str {
    match t {
        TextType::Plain => "plain",
        TextType::Html => "html",
    }
}

/// The text type of a stored word.
pub fn text_type_of(word: &str) -> TaskResult<TextType> {
    match word {
        "plain" => Ok(TextType::Plain),
        "html" => Ok(TextType::Html),
        _ => Err(adapt(damaged("description type"))),
    }
}

/// Finished in any way: completed, cancelled or failed.
#[must_use]
pub fn is_done(p: Progress) -> bool {
    matches!(
        p,
        Progress::Completed | Progress::Failed | Progress::Cancelled
    )
}

/// The stored word of a dependency kind (RFC 9253 `RELTYPE`).
#[must_use]
pub fn kind_word(k: DependencyKind) -> &'static str {
    match k {
        DependencyKind::FinishToStart => "finish-to-start",
        DependencyKind::FinishToFinish => "finish-to-finish",
        DependencyKind::StartToStart => "start-to-start",
        DependencyKind::StartToFinish => "start-to-finish",
        DependencyKind::DependsOn => "depends-on",
    }
}

/// The dependency kind of a stored word.
pub fn kind_of(word: &str) -> TaskResult<DependencyKind> {
    Ok(match word {
        "finish-to-start" => DependencyKind::FinishToStart,
        "finish-to-finish" => DependencyKind::FinishToFinish,
        "start-to-start" => DependencyKind::StartToStart,
        "start-to-finish" => DependencyKind::StartToFinish,
        "depends-on" => DependencyKind::DependsOn,
        _ => return Err(adapt(damaged("dependency kind"))),
    })
}

/// The stored word of what an offset trigger counts from.
#[must_use]
pub fn relative_word(r: RelativeTo) -> &'static str {
    match r {
        RelativeTo::Start => "start",
        RelativeTo::End => "end",
    }
}

/// What an offset trigger counts from, of a stored word.
pub fn relative_of(word: &str) -> TaskResult<RelativeTo> {
    match word {
        "start" => Ok(RelativeTo::Start),
        "end" => Ok(RelativeTo::End),
        _ => Err(adapt(damaged("alert relative to"))),
    }
}

/// The stored word of what an alert does.
#[must_use]
pub fn action_word(a: AlertAction) -> &'static str {
    match a {
        AlertAction::Display => "display",
        AlertAction::Email => "email",
    }
}

/// What an alert does, of a stored word.
pub fn action_of(word: &str) -> TaskResult<AlertAction> {
    match word {
        "display" => Ok(AlertAction::Display),
        "email" => Ok(AlertAction::Email),
        _ => Err(adapt(damaged("alert action"))),
    }
}
