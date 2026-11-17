//! Rules for the purge of a parent with a kept subtask: the kept one is not deleted with it, it is detached and stays
//! in the trash (`expire` does the same when the deadline passes; the trait text says "abgehängt, nicht mitgelöscht").
//!
//! Pattern: Template method (rules as plain checks collected in the report).

use super::ConformanceFixture;
use super::cx::Cx;
use crate::tasks_v1::{SeriesScope, Task, TaskPatch, Trash};

async fn kid_of(
    cx: &mut Cx<'_>,
    list: crate::tasks_v1::ObjectId,
    p: &Task,
    name: &str,
) -> Option<Task> {
    let rest = TaskPatch {
        parent: Some(Some(p.id)),
        ..TaskPatch::default()
    };
    cx.task_with(list, name, rest).await
}

/// A parent with a kept subtask (and a subtask below it) goes with `purge`: the plain subtask goes, the kept one is
/// detached with what hangs below it.
pub(super) async fn rules(fx: &dyn ConformanceFixture, cx: &mut Cx<'_>) {
    let Some(l) = cx.list("purge kept kid").await else {
        return;
    };
    let dir = cx.dir;
    let Some(p) = cx.task(l.id, "parent of a kept one").await else {
        return;
    };
    let (Some(kept), Some(plain)) = (
        kid_of(cx, l.id, &p, "kept kid").await,
        kid_of(cx, l.id, &p, "plain kid").await,
    ) else {
        return;
    };
    let Some(below) = kid_of(cx, l.id, &kept, "below the kept kid").await else {
        return;
    };
    if !fx.keep(kept.id, None).await {
        cx.skip("purge: a kept subtask is detached (the fixture cannot keep a task)");
        return;
    }
    let Some(p) = cx.fresh(p.id).await else {
        return;
    };
    let Ok(d) = dir.delete(p.id, p.revision, SeriesScope::Whole).await else {
        return;
    };
    cx.ensure(
        dir.purge(d.id).await.is_ok(),
        "purge: a parent with a kept subtask goes (the kept one does not stop it)",
    );
    let mut q = cx.query(&[l.id]);
    q.trash = Trash::Include;
    let all = cx.ids(&q).await;
    cx.ensure(
        all.contains(&kept.id) && all.contains(&below.id) && !all.contains(&plain.id),
        "purge: a kept subtask that went with the parent is detached, not deleted (with what hangs below it); the plain one goes",
    );
    let back = dir.restore(kept.id, SeriesScope::Whole).await;
    cx.ensure(
        back.is_ok_and(|b| b.parent.is_none())
            && dir
                .get(below.id)
                .await
                .ok()
                .flatten()
                .is_some_and(|b| b.parent == Some(kept.id)),
        "purge: the detached kept subtask comes back on its own, top-level, with its subtask",
    );
}
