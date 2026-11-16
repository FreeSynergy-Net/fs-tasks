//! The empty task: every field at its neutral value.
//!
//! Pattern: Builder (a starting value for `create` and for the sample).

use crate::tasks_v1::{ObjectId, Progress, Rank, RepeatFrom, Revision, Task, TextType, Utc};

/// A task with only id, list, `uid`, title and creation time set; rank `n` (the middle of an empty sequence).
///
/// # Panics
/// Never in practice: the rank `n` is a valid rank.
#[must_use]
pub fn blank_task(id: ObjectId, list: ObjectId, uid: &str, title: &str, at: Utc) -> Task {
    Task {
        id,
        list,
        uid: uid.to_owned(),
        revision: Revision(1),
        sequence: 0,
        created: at,
        updated: at,
        title: title.to_owned(),
        description: String::new(),
        description_type: TextType::Plain,
        locale: None,
        due: None,
        start: None,
        estimated: None,
        progress: Progress::NeedsAction,
        progress_updated: None,
        percent_complete: None,
        priority: 0,
        color: None,
        labels: Vec::new(),
        assignees: Vec::new(),
        alerts: Vec::new(),
        links: Vec::new(),
        parent: None,
        dependencies: Vec::new(),
        related: Vec::new(),
        recurrence: Vec::new(),
        repeat_from: RepeatFrom::Schedule,
        excluded_recurrence: Vec::new(),
        overrides: Vec::new(),
        checklist: Vec::new(),
        rank: Rank::parse("n").expect("`n` is a rank"),
        claim: None,
        origin: None,
    }
}
