//! `progress`: the board columns. A finished state ends the open subtasks according to `Children`; a moved task goes on
//! top of its new column. A task without a series takes only the scope `Whole` (the series of run 2b: `This`, from
//! completion, the last one ends the task).
//!
//! Pattern: Transaction script (in the caller's unit of work).

use fs_objects::v5;
use fs_objects::v5::codec::id_value;
use fs_objects::v5::envelope::bump;
use fs_tasks_testdata::tasks_v1::{
    Children, ListAccess, ObjectId, Progress, Revision, SeriesScope, Task, TaskError, TaskResult,
    Utc,
};

use crate::access::need;
use crate::error::{adapt, changed};
use crate::rank::top;
use crate::stored::{Stored, load_one, update_row};
use crate::tasks_write::{live_task, whole_only};
use crate::tree::Tree;
use crate::who::Who;
use crate::words::is_done;

/// The open subtasks below a task (all levels, level by level), only when there are any.
async fn open_below(c: &v5::Conn<'_>, s: &Stored) -> TaskResult<Vec<Stored>> {
    let any = c
        .query(
            "SELECT 1 FROM tasks_task WHERE parent = ? LIMIT 1",
            vec![id_value(s.task.id)],
        )
        .await
        .map_err(adapt)?;
    if any.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for kid in Tree::load(c, s.task.list).await?.descendants(s.task.id) {
        if let Some(k) = load_one(c, kid)
            .await?
            .filter(|k| !is_done(k.task.progress))
        {
            out.push(k);
        }
    }
    Ok(out)
}

/// One task gets the state: the time, the top of the new column, a new revision.
async fn set_one(c: &v5::Conn<'_>, now: i64, old: &Task, to: Progress) -> TaskResult<()> {
    let mut new = old.clone();
    new.progress = to;
    new.progress_updated = Some(Utc::from_millis(now));
    new.rank = top(c, now, old.list, old.parent, to, Some(old.id)).await?;
    bump(c, old.id, old.revision, None, now)
        .await
        .map_err(adapt)?;
    update_row(c, &new, None).await
}

/// `progress`: `NotFound`, `Forbidden`, `Changed`, then `Invalid` (the scope) and `OpenSubtasks` for `Refuse`.
#[allow(clippy::too_many_arguments)]
pub async fn progress(
    c: &v5::Conn<'_>,
    who: &Who,
    now: i64,
    id: ObjectId,
    expected: Revision,
    to: Progress,
    children: Children,
    scope: SeriesScope,
) -> TaskResult<Task> {
    let s = live_task(c, id).await?;
    need(c, who, s.task.list, ListAccess::Write, "task.progress").await?;
    if s.task.revision != expected {
        return Err(changed(s.task.revision));
    }
    whole_only(scope)?;
    let open = if is_done(to) {
        open_below(c, &s).await?
    } else {
        Vec::new()
    };
    match children {
        Children::Refuse if !open.is_empty() => {
            return Err(TaskError::OpenSubtasks(
                open.iter().map(|k| k.task.id).collect(),
            ));
        }
        Children::Same => {
            for k in &open {
                set_one(c, now, &k.task, to).await?;
            }
        }
        _ => {}
    }
    if s.task.progress != to {
        set_one(c, now, &s.task, to).await?;
    }
    Ok(live_task(c, id).await?.task)
}
