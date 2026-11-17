//! The trash of a task: delete (with its living subtasks), restore, purge, and the deadline run.
//!
//! Pattern: Transaction scripts (in the caller's unit of work). A task deleted together with its parent carries
//! `deleted_with`; a task of a deleted list carries nothing (its state is the list's). Final delete goes through the
//! layer's `delete_final`, which follows every `parent` and `list` row (`registry::CHILDREN`): so a subtask that was
//! deleted EARLIER on its own is hung off its parent first (`detach`, PLAN O18), in the same transaction, and keeps its
//! own deadline.

use fs_objects::v5;
use fs_objects::v5::codec::{get_text, id_value};
use fs_objects::v5::envelope::{delete_final, due, state, trash, untrash};
use fs_tasks_testdata::tasks_v1::{
    Deleted, ListAccess, ObjectId, Revision, SUBTASK_DEPTH_MAX, SeriesScope, Task, TaskResult,
};

use crate::access::need;
use crate::error::{adapt, bad, changed, gone};
use crate::lists::any;
use crate::registry::DOMAIN;
use crate::stored::{load_one, marks};
use crate::tasks_write::{live_task, whole_only};
use crate::trash_state::Gone;
use crate::tree::Tree;
use crate::who::Who;

/// `delete`: the task goes into the trash with its living subtasks, one deadline.
pub async fn delete(
    c: &v5::Conn<'_>,
    who: &Who,
    now: i64,
    id: ObjectId,
    expected: Revision,
    scope: SeriesScope,
) -> TaskResult<Deleted> {
    let s = live_task(c, id).await?;
    need(c, who, s.task.list, ListAccess::Write, "task.delete").await?;
    if s.task.revision != expected {
        return Err(changed(s.task.revision));
    }
    whole_only(scope)?;
    let kids = Tree::load(c, s.task.list).await?.descendants(id);
    let (_, until) = trash(c, id, expected, now).await.map_err(adapt)?;
    for kid in &kids {
        c.exec(
            "UPDATE tasks_task SET deleted_with = ? WHERE id = ?",
            vec![id_value(id), id_value(*kid)],
        )
        .await
        .map_err(adapt)?;
    }
    Ok(Deleted {
        id,
        scope: SeriesScope::Whole,
        with_subtasks: kids,
        restorable: Some(
            Gone {
                deleted: now,
                until,
            }
            .restorable(),
        ),
    })
}

/// `restore`: `NotFound` (also after the deadline), then `Invalid` (not deleted, deleted with its parent or its list,
/// the parent is in the trash, the tree would be too deep); nothing is written on a refusal.
pub async fn restore(
    c: &v5::Conn<'_>,
    who: &Who,
    now: i64,
    id: ObjectId,
    scope: SeriesScope,
) -> TaskResult<Task> {
    let s = load_one(c, id).await?.ok_or_else(|| gone(id))?;
    need(c, who, s.task.list, ListAccess::Write, "task.restore").await?;
    whole_only(scope)?;
    if s.trash().is_none() {
        return Err(bad("task", "not-deleted"));
    }
    if s.own.is_none() {
        return Err(bad(
            "task",
            if s.with.is_some() {
                "deleted-with-parent"
            } else {
                "list-in-trash"
            },
        ));
    }
    if s.list.is_some() {
        return Err(bad("task", "list-in-trash"));
    }
    if let Some(p) = s.task.parent
        && load_one(c, p).await?.is_some_and(|ps| ps.trash().is_some())
    {
        return Err(bad("parent", "in-trash"));
    }
    let tree = Tree::load(c, s.task.list).await?;
    let above = s.task.parent.map_or(0, |p| tree.depth_of(p));
    if above + 1 + tree.below_with(id) > SUBTASK_DEPTH_MAX {
        return Err(bad("parent", "too-deep"));
    }
    untrash(c, id, now).await.map_err(adapt)?;
    c.exec(
        "UPDATE tasks_task SET deleted_with = NULL WHERE deleted_with = ?",
        vec![id_value(id)],
    )
    .await
    .map_err(adapt)?;
    Ok(load_one(c, id).await?.ok_or_else(|| gone(id))?.task)
}

/// `purge`: a task (with the subtasks that went with it) or a list (with all its tasks) in the trash goes now.
pub async fn purge(c: &v5::Conn<'_>, who: &Who, id: ObjectId) -> TaskResult<()> {
    let kind = kind_of(c, id).await?.ok_or_else(|| gone(id))?;
    if kind == "list" {
        need(c, who, id, ListAccess::Manage, "trash.purge").await?;
        if any(c, id).await?.trash.is_none() {
            return Err(bad("id", "not-in-trash"));
        }
    } else {
        let s = load_one(c, id).await?.ok_or_else(|| gone(id))?;
        need(c, who, s.task.list, ListAccess::Manage, "trash.purge").await?;
        if s.trash().is_none() {
            return Err(bad("id", "not-in-trash"));
        }
        if s.own.is_none() {
            return Err(bad(
                "id",
                if s.with.is_some() {
                    "deleted-with-parent"
                } else {
                    "list-in-trash"
                },
            ));
        }
        detach(c, id).await?;
    }
    delete_final(c, &DOMAIN, id).await.map_err(adapt)
}

/// The deadline run: every object whose trash period is over goes for good, with what hangs on it.
pub async fn sweep(c: &v5::Conn<'_>, now: i64) -> TaskResult<usize> {
    let mut n = 0;
    for id in due(c, now).await.map_err(adapt)? {
        if state(c, id).await.map_err(adapt)?.is_none() {
            continue;
        }
        if kind_of(c, id).await?.is_some_and(|k| k == "task") {
            detach(c, id).await?;
        }
        delete_final(c, &DOMAIN, id).await.map_err(adapt)?;
        n += 1;
    }
    Ok(n)
}

/// The kind of an object (`list` or `task`), `None` when there is no such object.
async fn kind_of(c: &v5::Conn<'_>, id: ObjectId) -> TaskResult<Option<String>> {
    let rows = c
        .query("SELECT type FROM objects WHERE id = ?", vec![id_value(id)])
        .await
        .map_err(adapt)?;
    rows.first()
        .map(|r| get_text(r, 0).map_err(adapt))
        .transpose()
}

/// PLAN O18: the task and the subtasks that went with it leave together; every other task that still hangs below one
/// of them (deleted earlier on its own, so with its own deadline) loses its parent and becomes a top-level task.
async fn detach(c: &v5::Conn<'_>, id: ObjectId) -> TaskResult<()> {
    let rows = c
        .query(
            "SELECT id FROM tasks_task WHERE deleted_with = ?",
            vec![id_value(id)],
        )
        .await
        .map_err(adapt)?;
    let mut members = vec![id];
    for r in &rows {
        members.push(fs_objects::v5::codec::get_id(r, 0).map_err(adapt)?);
    }
    let ids: Vec<_> = members.iter().map(|m| id_value(*m)).collect();
    let sql = format!(
        "UPDATE tasks_task SET parent = NULL WHERE parent IN ({m}) AND id NOT IN ({m})",
        m = marks(ids.len())
    );
    let mut params = ids.clone();
    params.extend(ids);
    c.exec(&sql, params).await.map_err(adapt)?;
    Ok(())
}
