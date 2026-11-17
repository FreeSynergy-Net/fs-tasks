//! The short form of a task (`TaskSummary`) for list, board and "Heute".
//!
//! Pattern: Value object builder (a pure view of one stored row; a series shows its current occurrence).

use crate::ready::is_done;
use crate::series::current;
use crate::state::State;
use crate::tasks_v1::TaskSummary;

impl State {
    /// The summary of the row `i`, with the trash deadline of the task or of its list.
    pub(crate) fn summary(&self, i: usize) -> TaskSummary {
        let t = &self.rows[i].task;
        let cur = current(t);
        let kids = self.children(t.id);
        let total = u32::try_from(kids.len()).unwrap_or(u32::MAX);
        let open = u32::try_from(
            kids.iter()
                .filter(|&&k| !is_done(self.rows[k].task.progress))
                .count(),
        )
        .unwrap_or(u32::MAX);
        let claimed_by = t
            .claim
            .as_ref()
            .filter(|c| c.until > self.now)
            .map(|c| c.by.clone());
        TaskSummary {
            id: t.id,
            list: t.list,
            uid: t.uid.clone(),
            title: t.title.clone(),
            due: cur.due,
            start: cur.start,
            occurrence: cur.occurrence,
            recurring: !t.recurrence.is_empty(),
            progress: cur.progress,
            priority: t.priority,
            labels: t.labels.clone(),
            assignees: cur
                .assignees
                .iter()
                .map(|a| (a.who.clone(), a.status))
                .collect(),
            parent: t.parent,
            subtasks_open: open,
            subtasks_total: total,
            readiness: self.readiness_of(i),
            rank: t.rank.clone(),
            claimed_by,
            revision: t.revision,
            trash: self.trash_of(i),
            has_description: !t.description.is_empty(),
            percent_complete: t.percent_complete,
            estimated: t.estimated,
            links: u32::try_from(t.links.len()).unwrap_or(u32::MAX),
            alerts: u32::try_from(t.alerts.len()).unwrap_or(u32::MAX),
            checklist_open: u32::try_from(t.checklist.iter().filter(|c| c.done.is_none()).count())
                .unwrap_or(u32::MAX),
            checklist_total: u32::try_from(t.checklist.len()).unwrap_or(u32::MAX),
        }
    }
}
