//! The short form of a task (`TaskSummary`) for list, board and "Heute".
//!
//! Pattern: Value object builder (a pure view of one stored task and the counts of its subtasks).

use std::collections::BTreeMap;

use fs_objects::v5;
use fs_objects::v5::codec::id_value;
use fs_tasks_testdata::tasks_v1::{ObjectId, Readiness, TaskResult, TaskSummary};

use crate::query::ALIVE;
use crate::stored::{Stored, load_with, marks};
use crate::words::is_done;

/// Open and all living subtasks, per parent.
pub type Counts = BTreeMap<ObjectId, (u32, u32)>;

/// The subtask counts of the tasks `ids` (one `SELECT` over their living subtasks).
pub async fn counts_of(c: &v5::Conn<'_>, ids: &[ObjectId]) -> TaskResult<Counts> {
    let mut out = Counts::new();
    for chunk in ids.chunks(400) {
        let cond = format!("t.parent IN ({}) AND {ALIVE}", marks(chunk.len()));
        let params = chunk.iter().map(|i| id_value(*i)).collect();
        for s in load_with(c, &cond, params, "ORDER BY t.id").await? {
            if let Some(p) = s.task.parent {
                let e = out.entry(p).or_default();
                e.1 += 1;
                if !is_done(s.task.progress) {
                    e.0 += 1;
                }
            }
        }
    }
    Ok(out)
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
        checklist_open: u32::try_from(t.checklist.iter().filter(|i| i.done.is_none()).count())
            .unwrap_or(u32::MAX),
        checklist_total: u32::try_from(t.checklist.len()).unwrap_or(u32::MAX),
    }
}
