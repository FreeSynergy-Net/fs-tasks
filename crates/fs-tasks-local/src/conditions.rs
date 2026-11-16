//! The conditions of a query that the database decides: the lists, the trash view, the progress, the parent, the labels, the
//! undated flag, the day range (`due_sql`) and the ready filter (`ready_sql`), each one a small function that adds its text
//! and its parameters.
//!
//! Pattern: Specification (a query is a set of conditions, joined by `AND`; the parameters follow the marks in order).

use chrono_tz::Tz;
use fs_objects::v5::codec::{id_value, text};
use fs_tasks_testdata::tasks_v1::{AssigneeFilter, ObjectId, ParentFilter, TaskQuery, Trash};
use turso::Value;

use crate::query::ALIVE;
use crate::stored::marks;
use crate::words::progress_word;
use crate::{due_sql, ready_sql};

/// A task that is not finished (the states a task can still be ready in).
const OPEN: &str = "t.progress IN ('needs-action', 'in-process')";

/// The conditions of the query over the lists `lists` (the ones the asker may see), with their parameters. `deps`: some
/// living task holds a dependency (without one the ready filter is only "open"). `None`: nothing can match (an assignee
/// filter, since this provider stores no assignees yet).
#[must_use]
pub fn build(
    q: &TaskQuery,
    lists: &[ObjectId],
    view: Tz,
    deps: bool,
) -> Option<(String, Vec<Value>)> {
    if !matches!(q.assignee, None | Some(AssigneeFilter::Nobody)) {
        return None;
    }
    let mut cond = vec![format!("t.list IN ({})", marks(lists.len()))];
    let mut params: Vec<Value> = lists.iter().map(|l| id_value(*l)).collect();
    match q.trash {
        Trash::Exclude => cond.push(ALIVE.to_owned()),
        Trash::Only => cond.push(format!("NOT ({ALIVE})")),
        Trash::Include => {}
    }
    if !q.progress.is_empty() {
        cond.push(format!("t.progress IN ({})", marks(q.progress.len())));
        params.extend(q.progress.iter().map(|p| text(progress_word(*p))));
    }
    match q.parent {
        None => {}
        Some(ParentFilter::TopLevel) => cond.push("t.parent IS NULL".to_owned()),
        Some(ParentFilter::Of(p)) => {
            cond.push("t.parent = ?".to_owned());
            params.push(id_value(p));
        }
    }
    if let Some(u) = q.undated {
        let none = "t.due_kind IS NULL AND t.start_kind IS NULL";
        cond.push(if u {
            none.to_owned()
        } else {
            format!("NOT ({none})")
        });
    }
    for l in &q.labels {
        cond.push(
            "EXISTS (SELECT 1 FROM tasks_label lb WHERE lb.task = t.id AND lb.value = ?)"
                .to_owned(),
        );
        params.push(text(l));
    }
    if let Some(r) = &q.due {
        let (text, values) = due_sql::condition(r, view);
        cond.push(text);
        params.extend(values);
    }
    if let Some(want) = q.ready {
        let ready = if deps {
            ready_sql::ready()
        } else {
            format!("({OPEN})")
        };
        cond.push(if want { ready } else { format!("NOT {ready}") });
    }
    Some((cond.join(" AND "), params))
}
