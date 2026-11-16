//! The short form of a task (`TaskSummary`) for list, board and "Heute".
//!
//! Pattern: Value object builder (a pure view of one stored task and the counts of its subtasks).

use std::collections::BTreeMap;

use fs_tasks_testdata::tasks_v1::{ObjectId, Readiness, TaskSummary};

use crate::stored::Stored;
use crate::words::is_done;

/// Open and all living subtasks, per parent.
pub type Counts = BTreeMap<ObjectId, (u32, u32)>;

/// The subtask counts of every task in `all`.
#[must_use]
pub fn counts(all: &[Stored]) -> Counts {
    let mut out = Counts::new();
    for s in all.iter().filter(|s| s.alive()) {
        if let Some(p) = s.task.parent {
            let e = out.entry(p).or_default();
            e.1 += 1;
            if !is_done(s.task.progress) {
                e.0 += 1;
            }
        }
    }
    out
}

/// The summary of one task. Without `dependency.evaluate` (not offered here) a task without dependencies is `Ready`,
/// or `Done` once it is finished; this run stores no dependencies.
#[must_use]
pub fn summary(s: &Stored, counts: &Counts) -> TaskSummary {
    let t = &s.task;
    let (open, total) = counts.get(&t.id).copied().unwrap_or_default();
    TaskSummary {
        id: t.id,
        list: t.list,
        uid: t.uid.clone(),
        title: t.title.clone(),
        due: t.due.clone(),
        start: t.start.clone(),
        occurrence: None,
        recurring: false,
        progress: t.progress,
        priority: t.priority,
        labels: t.labels.clone(),
        assignees: Vec::new(),
        parent: t.parent,
        subtasks_open: open,
        subtasks_total: total,
        readiness: if is_done(t.progress) {
            Readiness::Done
        } else {
            Readiness::Ready
        },
        rank: t.rank.clone(),
        claimed_by: None,
        revision: t.revision,
        trash: s.restorable(),
        has_description: !t.description.is_empty(),
        percent_complete: t.percent_complete,
        estimated: t.estimated,
        links: 0,
        alerts: 0,
        checklist_open: 0,
        checklist_total: 0,
    }
}
