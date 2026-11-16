//! `relate`, `unrelate`: the dependencies of a task (RFC 9253), with the cycle check, and `readiness` for a set of tasks.
//!
//! Pattern: Transaction scripts (check everything first, then write; the caller owns commit and rollback). The cycle check
//! runs in the same transaction as the write, and `begin` takes the write lock, so two writers cannot close a cycle
//! between them.

use fs_objects::v5;
use fs_objects::v5::codec::{get_int, id_value};
use fs_objects::v5::envelope::bump;
use fs_tasks_testdata::tasks_v1::{
    Dependency, DependencyKind, ListAccess, ObjectId, Readiness, Revision, Task, TaskError,
    TaskResult,
};

use crate::access::{level, need};
use crate::deps_rows::{add, remove};
use crate::error::{adapt, bad, changed, gone};
use crate::graph::Graph;
use crate::stored::{load_one, load_with, marks};
use crate::tasks_write::live_task;
use crate::who::Who;

/// `relate`: `NotFound`, `Forbidden`, `Changed`, then `Invalid` (a gap on `DependsOn`, an empty `uid`), `Cycle`; a tie the
/// task holds already is no change (no new revision).
pub async fn relate(
    c: &v5::Conn<'_>,
    who: &Who,
    now: i64,
    id: ObjectId,
    expected: Revision,
    dep: Dependency,
) -> TaskResult<Task> {
    let s = live_task(c, id).await?;
    need(c, who, s.task.list, ListAccess::Write, "relation.create").await?;
    if s.task.revision != expected {
        return Err(changed(s.task.revision));
    }
    if dep.kind == DependencyKind::DependsOn && dep.gap.is_some() {
        return Err(bad("gap", "not-with-depends-on"));
    }
    if dep.other_uid.is_empty() {
        return Err(bad("other_uid", "empty"));
    }
    let uid = s.task.uid.clone();
    if dep.other_uid == uid {
        return Err(TaskError::Cycle(vec![uid]));
    }
    if s.task
        .dependencies
        .iter()
        .any(|d| d.other_uid == dep.other_uid && d.kind == dep.kind)
    {
        return Ok(s.task);
    }
    let graph = Graph::load(c, false).await?;
    if let Some(path) = graph.find_cycle(&uid, Some((&uid, &dep.other_uid, dep.kind))) {
        return Err(TaskError::Cycle(path));
    }
    let rows = c
        .query(
            "SELECT COALESCE(MAX(pos) + 1, 0) FROM tasks_dependency WHERE task = ?",
            vec![id_value(id)],
        )
        .await
        .map_err(adapt)?;
    let pos = rows
        .first()
        .map_or(Ok(0), |r| get_int(r, 0))
        .map_err(adapt)?;
    add(c, id, pos, &dep).await?;
    bump(c, id, expected, Some(&s.task.title), now)
        .await
        .map_err(adapt)?;
    Ok(load_one(c, id).await?.ok_or_else(|| gone(id))?.task)
}

/// `unrelate`: `NotFound`, `Forbidden`, `Changed`, `Invalid` when the task holds no such tie.
pub async fn unrelate(
    c: &v5::Conn<'_>,
    who: &Who,
    now: i64,
    id: ObjectId,
    expected: Revision,
    other_uid: &str,
    kind: DependencyKind,
) -> TaskResult<Task> {
    let s = live_task(c, id).await?;
    need(c, who, s.task.list, ListAccess::Write, "relation.delete").await?;
    if s.task.revision != expected {
        return Err(changed(s.task.revision));
    }
    if !s
        .task
        .dependencies
        .iter()
        .any(|d| d.other_uid == other_uid && d.kind == kind)
    {
        return Err(bad("other_uid", "not-related"));
    }
    remove(c, id, other_uid, kind).await?;
    bump(c, id, expected, Some(&s.task.title), now)
        .await
        .map_err(adapt)?;
    Ok(load_one(c, id).await?.ok_or_else(|| gone(id))?.task)
}

/// `readiness`: one value per id, in the order asked; an id that is not a living task the asker may see is `NotFound`
/// for the whole call.
pub async fn readiness(
    c: &v5::Conn<'_>,
    who: &Who,
    ids: &[ObjectId],
) -> TaskResult<Vec<(ObjectId, Readiness)>> {
    let mut found = Vec::new();
    for chunk in ids.chunks(400) {
        let cond = format!("t.id IN ({})", marks(chunk.len()));
        let params = chunk.iter().map(|i| id_value(*i)).collect();
        found.extend(load_with(c, &cond, params, "ORDER BY t.id").await?);
    }
    let graph = Graph::load(c, true).await?;
    let mut out = Vec::new();
    for id in ids {
        let s = found
            .iter()
            .find(|s| s.task.id == *id && s.alive())
            .ok_or_else(|| gone(*id))?;
        if level(c, who, s.task.list).await?.is_none() {
            return Err(gone(*id));
        }
        out.push((*id, graph.readiness(&s.task.uid, s.task.progress)));
    }
    Ok(out)
}
