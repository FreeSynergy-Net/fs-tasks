//! Tasks: create and update (the stored fields), with the rules of the trait (`Invalid`, `Cycle`, depth, scope).
//!
//! Pattern: Transaction scripts (check everything first, then write; the caller owns commit and rollback).

use fs_objects::v5;
use fs_objects::v5::codec::{id_text, id_value};
use fs_objects::v5::envelope::{bump, insert, touch};
use fs_objects::v5::new_id;
use fs_tasks_testdata::blank_task;
use fs_tasks_testdata::tasks_v1::{
    ListAccess, ObjectId, Revision, SUBTASK_DEPTH_MAX, SeriesScope, Task, TaskDraft, TaskError,
    TaskPatch, TaskResult, Utc,
};

use crate::access::need;
use crate::checklist_rows::create_items;
use crate::error::{adapt, bad, changed, gone, unsupported};
use crate::lists::live;
use crate::patch::{apply, check};
use crate::rank::top;
use crate::stored::{Stored, load_one, row_values, update_row, write_labels};
use crate::tables::TASK;
use crate::tree::Tree;
use crate::who::Who;
use crate::words::is_done;
use crate::{alerts_rows, day, links_rows};

/// A living task (not deleted, not with a parent, its list alive); `NotFound` otherwise.
pub async fn live_task(c: &v5::Conn<'_>, id: ObjectId) -> TaskResult<Stored> {
    load_one(c, id)
        .await?
        .filter(Stored::alive)
        .ok_or_else(|| gone(id))
}

/// The series of a task is a run 2 matter: this run has no recurrence, so every scope but `Whole` is `Invalid`.
pub fn whole_only(scope: SeriesScope) -> TaskResult<()> {
    if scope == SeriesScope::Whole {
        Ok(())
    } else {
        Err(bad("scope", "not-a-series"))
    }
}

/// `create`.
pub async fn create(c: &v5::Conn<'_>, who: &Who, now: i64, d: TaskDraft) -> TaskResult<Task> {
    need(c, who, d.list, ListAccess::Write, "task.create").await?;
    if d.rest.title.is_some() || d.rest.list.is_some() {
        return Err(bad("rest", "title-or-list-set-twice"));
    }
    if d.title.trim().is_empty() {
        return Err(bad("title", "empty"));
    }
    live(c, d.list).await?;
    check(&d.rest)?;
    if !d.assignees.is_empty() {
        return Err(unsupported(
            "task.create",
            "assignees are stored in K1 run 2b (they need the groups)",
        ));
    }
    let parent = d.rest.parent.flatten();
    if let Some(p) = parent {
        let tree = Tree::load(c, d.list).await?;
        let node = tree.node(p).filter(|n| n.alive);
        if node.is_none() {
            return Err(parent_error(c, p, d.list).await?);
        }
        if tree.depth_of(p) + 1 > SUBTASK_DEPTH_MAX {
            return Err(bad("parent", "too-deep"));
        }
    }
    let id = new_id(now).map_err(adapt)?;
    if let Some(u) = &d.uid
        && (u.is_empty() || uid_taken(c, u).await?)
    {
        return Err(bad("uid", "exists-or-empty"));
    }
    let uid = d.uid.unwrap_or_else(|| format!("urn:uuid:{}", id_text(id)));
    let mut t = blank_task(id, d.list, &uid, &d.title, Utc::from_millis(now));
    t.progress = d.progress;
    t.parent = parent;
    apply(&mut t, d.rest);
    if is_done(t.progress) {
        t.progress_updated = Some(Utc::from_millis(now));
    }
    t.rank = top(c, now, t.list, t.parent, t.progress, None).await?;
    insert(c, id, "task", &t.title, &who.me, now)
        .await
        .map_err(adapt)?;
    c.insert(&TASK, row_values(&t, None)?)
        .await
        .map_err(adapt)?;
    write_labels(c, id, &t.labels).await?;
    create_items(c, id, &d.checklist).await?;
    write_parts(c, &t, None).await?;
    Ok(load_one(c, id).await?.ok_or_else(|| gone(id))?.task)
}

/// The tables of a task that its fields feed: the alerts, the links and the derived day columns. Only what changed is
/// written (`old` is `None` for a new task: everything).
async fn write_parts(c: &v5::Conn<'_>, new: &Task, old: Option<&Task>) -> TaskResult<()> {
    if old.is_none_or(|o| o.alerts != new.alerts) {
        alerts_rows::write(c, new.id, &new.alerts).await?;
    }
    if old.is_none_or(|o| o.links != new.links) {
        links_rows::write(c, new.id, &new.links).await?;
    }
    if old.is_none_or(|o| o.due != new.due || o.start != new.start) {
        day::write(c, new).await?;
    }
    Ok(())
}

async fn uid_taken(c: &v5::Conn<'_>, uid: &str) -> TaskResult<bool> {
    let rows = c
        .query(
            "SELECT 1 FROM tasks_task WHERE uid = ?",
            vec![fs_objects::v5::codec::text(uid)],
        )
        .await
        .map_err(adapt)?;
    Ok(!rows.is_empty())
}

/// Why a named parent does not do: not there or in the trash is `NotFound`, in another list is `Invalid`.
async fn parent_error(c: &v5::Conn<'_>, p: ObjectId, list: ObjectId) -> TaskResult<TaskError> {
    Ok(match load_one(c, p).await? {
        Some(s) if s.alive() && s.task.list != list => bad("parent", "other-list"),
        _ => gone(p),
    })
}

/// `update`: the stored fields; moving to another list or under another task moves the whole subtree.
pub async fn update(
    c: &v5::Conn<'_>,
    who: &Who,
    now: i64,
    id: ObjectId,
    expected: Revision,
    patch: TaskPatch,
    scope: SeriesScope,
) -> TaskResult<Task> {
    let stored = live_task(c, id).await?;
    need(c, who, stored.task.list, ListAccess::Write, "task.edit").await?;
    let old = stored.task.clone();
    if old.revision != expected {
        return Err(changed(old.revision));
    }
    check(&patch)?;
    whole_only(scope)?;
    let list = patch.list.unwrap_or(old.list);
    let parent = patch.parent.unwrap_or(old.parent);
    if list != old.list {
        need(c, who, list, ListAccess::Write, "task.edit").await?;
        live(c, list).await?;
    }
    let old_tree = Tree::load(c, old.list).await?;
    if let Some(p) = parent {
        let tree = if list == old.list {
            old_tree.clone()
        } else {
            Tree::load(c, list).await?
        };
        check_parent(c, &tree, &old_tree, &old, list, p).await?;
    }
    let mut new = old.clone();
    apply(&mut new, patch);
    let moved = list != old.list || parent != old.parent;
    (new.list, new.parent) = (list, parent);
    if moved {
        new.rank = top(c, now, list, parent, new.progress, Some(id)).await?;
    }
    if new == old {
        return Ok(old);
    }
    bump(c, id, expected, Some(&new.title), now)
        .await
        .map_err(adapt)?;
    update_row(c, &new, None).await?;
    if new.labels != old.labels {
        write_labels(c, id, &new.labels).await?;
    }
    write_parts(c, &new, Some(&old)).await?;
    if list != old.list {
        for kid in old_tree.descendants(id) {
            c.exec(
                "UPDATE tasks_task SET list = ? WHERE id = ?",
                vec![id_value(list), id_value(kid)],
            )
            .await
            .map_err(adapt)?;
            touch(c, kid, None, now).await.map_err(adapt)?;
        }
    }
    Ok(load_one(c, id).await?.ok_or_else(|| gone(id))?.task)
}

/// The parent of an update must be a living task of the target list, no cycle, and the tree must stay shallow enough
/// (the whole subtree of the moved task counts).
async fn check_parent(
    c: &v5::Conn<'_>,
    tree: &Tree,
    old_tree: &Tree,
    old: &Task,
    list: ObjectId,
    p: ObjectId,
) -> TaskResult<()> {
    if p == old.id {
        return Err(TaskError::Cycle(vec![old.uid.clone()]));
    }
    if !tree.node(p).is_some_and(|n| n.alive) {
        return Err(parent_error(c, p, list).await?);
    }
    if let Some(path) = tree.cycle_path(old.id, p) {
        return Err(TaskError::Cycle(path));
    }
    if tree.depth_of(p) + old_tree.height_of(old.id) > SUBTASK_DEPTH_MAX {
        return Err(bad("parent", "too-deep"));
    }
    Ok(())
}
