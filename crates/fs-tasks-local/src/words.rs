//! The stored words of the enumerations (JSCalendar words) and back.
//!
//! Pattern: Adapter (value to column word and back; an unknown word in a row is `Unavailable`, never a guess).

use fs_objects::v5::codec::damaged;
use fs_tasks_testdata::tasks_v1::{Progress, TaskResult, TextType};

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
