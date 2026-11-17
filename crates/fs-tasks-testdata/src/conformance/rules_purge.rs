//! Rules for `purge` and the trash states: a task or list goes for good, the cascade, a subtask on its own (PLAN O18),
//! "never automatically" (`Manual`) and a kept one (`Held`).
//!
//! Pattern: Template method (rules as plain checks collected in the report).

use super::ConformanceFixture;
use super::cx::{Cx, is_forbidden, is_invalid, is_not_found};
use crate::tasks_v1::{ObjectId, Restorable, SeriesScope, Task, TaskPatch, Trash};

pub(super) async fn rules(cx: &mut Cx<'_>) {
    if !cx.caps("trash.purge") {
        cx.skip("purge: the provider has no trash.purge");
        return;
    }
    let Some(list) = cx.list("purge").await else {
        return;
    };
    let dir = cx.dir;
    let Some(t) = cx.task(list.id, "to purge").await else {
        return;
    };
    let living = dir.purge(t.id).await;
    cx.fails(
        &living,
        "purge: a task that is not in the trash is Invalid",
        is_invalid,
    );
    let gone = dir.purge(crate::ids::object_id(u64::MAX)).await;
    cx.fails(&gone, "purge: an unknown id is NotFound", is_not_found);
    let Ok(d) = dir.delete(t.id, t.revision, SeriesScope::Whole).await else {
        return;
    };
    cx.ensure(
        dir.purge(d.id).await.is_ok(),
        "purge: a task in the trash goes now",
    );
    let mut q = cx.query(&[list.id]);
    q.trash = Trash::Include;
    cx.ensure(
        cx.summary_in(&q, t.id).await.is_none(),
        "purge: the task is in no view any more",
    );
    cx.fails(
        &dir.restore(t.id, SeriesScope::Whole).await,
        "restore: a purged task is NotFound",
        is_not_found,
    );
    if cx.caps("task.history") {
        cx.fails(
            &dir.history(t.id, None, 10).await,
            "history: the log goes with the task (NotFound)",
            is_not_found,
        );
    }
    subtasks(cx, list.id).await;
    list_cascade(cx).await;
}

async fn kid_of(cx: &mut Cx<'_>, list: ObjectId, parent: &Task, title: &str) -> Option<Task> {
    cx.task_with(
        list,
        title,
        TaskPatch {
            parent: Some(Some(parent.id)),
            ..TaskPatch::default()
        },
    )
    .await
}

/// PLAN O18: the subtasks that went with the parent go with it; one deleted earlier on its own stays and becomes top-level.
async fn subtasks(cx: &mut Cx<'_>, list: ObjectId) {
    let dir = cx.dir;
    let Some(p) = cx.task(list, "parent").await else {
        return;
    };
    let (Some(with), Some(early)) = (
        kid_of(cx, list, &p, "with").await,
        kid_of(cx, list, &p, "early").await,
    ) else {
        return;
    };
    let Ok(early_gone) = dir
        .delete(early.id, early.revision, SeriesScope::Whole)
        .await
    else {
        return;
    };
    let Some(p2) = cx.fresh(p.id).await else {
        return;
    };
    let Ok(d) = dir.delete(p2.id, p2.revision, SeriesScope::Whole).await else {
        return;
    };
    cx.fails(
        &dir.purge(with.id).await,
        "purge: a subtask that was deleted with its parent is Invalid (purge the parent)",
        is_invalid,
    );
    cx.ensure(
        dir.purge(d.id).await.is_ok(),
        "purge: the parent goes with the subtasks that went with it",
    );
    let mut q = cx.query(&[list]);
    q.trash = Trash::Include;
    cx.ensure(
        cx.summary_in(&q, with.id).await.is_none() && cx.summary_in(&q, early_gone.id).await.is_some(),
        "purge: the subtask deleted earlier stays (own deadline), the one deleted with the parent goes",
    );
    let back = dir.restore(early.id, SeriesScope::Whole).await;
    cx.ensure(
        back.is_ok_and(|b| b.parent.is_none()),
        "purge: the subtask that stayed lost its parent (top-level), nothing was deleted through a parent row",
    );
}

/// A list that goes for good takes its tasks along, also those deleted on their own earlier.
async fn list_cascade(cx: &mut Cx<'_>) {
    let dir = cx.dir;
    let Some(l) = cx.list("purge list").await else {
        return;
    };
    let (Some(a), Some(b)) = (cx.task(l.id, "a").await, cx.task(l.id, "b").await) else {
        return;
    };
    let Ok(early) = dir.delete(b.id, b.revision, SeriesScope::Whole).await else {
        return;
    };
    let Ok(dl) = dir.delete_list(l.id, l.revision).await else {
        return;
    };
    cx.ensure(
        dir.purge(dl.id).await.is_ok(),
        "purge: a list in the trash goes now",
    );
    let lists = dir.lists(Trash::Include).await.unwrap_or_default();
    cx.ensure(
        !lists.iter().any(|x| x.id == l.id),
        "purge: the list is gone",
    );
    cx.ensure(
        dir.get(a.id).await.ok().flatten().is_none()
            && dir.restore(early.id, SeriesScope::Whole).await.is_err(),
        "purge: its tasks go with it, also the one deleted on its own earlier",
    );
    let mut q = cx.query(&[l.id]);
    q.trash = Trash::Include;
    cx.ensure(
        cx.all(&q).await.is_empty(),
        "purge: its tasks go with it (no view lists a task of the purged list)",
    );
}

/// "Never automatically" stays; a kept one is `Held`, cannot be purged, and keeps its list in the trash.
pub(super) async fn states(fx: &dyn ConformanceFixture, cx: &mut Cx<'_>) {
    let dir = cx.dir;
    if !cx.caps("trash.purge") {
        return;
    }
    let Some(l) = cx.list("trash states").await else {
        return;
    };
    let (Some(a), Some(held)) = (cx.task(l.id, "manual").await, cx.task(l.id, "held").await) else {
        return;
    };
    if fx.set_trash_days(None).await {
        let d = dir.delete(a.id, a.revision, SeriesScope::Whole).await;
        cx.ensure(
            d.as_ref()
                .is_ok_and(|d| d.restorable == Some(Restorable::Manual)),
            "delete: with the setting 'never automatically' the state is Manual",
        );
        if fx.advance_clock(900 * 86_400_000).await {
            cx.ensure(
                dir.restore(a.id, SeriesScope::Whole).await.is_ok(),
                "restore: a Manual task is still there after years",
            );
        }
        fx.set_trash_days(Some(30)).await;
    } else {
        cx.skip("trash: Manual (the fixture cannot change the trash setting)");
    }
    super::rules_purge_kept::rules(fx, cx).await;
    held_rules(fx, cx, l.id, &held).await;
}

async fn held_rules(fx: &dyn ConformanceFixture, cx: &mut Cx<'_>, list: ObjectId, t: &Task) {
    let dir = cx.dir;
    if !fx.keep(t.id, None).await {
        cx.skip("trash: Held (the fixture cannot keep a task)");
        return;
    }
    let Some(t) = cx.fresh(t.id).await else {
        return;
    };
    let Ok(d) = dir.delete(t.id, t.revision, SeriesScope::Whole).await else {
        return;
    };
    cx.ensure(
        d.restorable == Some(Restorable::Held(None)),
        "delete: a task kept without an end is Held(None)",
    );
    cx.fails(
        &dir.purge(t.id).await,
        "purge: a kept task is Forbidden",
        is_forbidden,
    );
    if fx.advance_clock(900 * 86_400_000).await {
        cx.ensure(
            dir.restore(t.id, SeriesScope::Whole).await.is_ok(),
            "trash: a kept task is still there long after the deadline",
        );
    }
    let Some(rev) = cx.fresh_list(list).await else {
        return;
    };
    let Ok(dl) = dir.delete_list(list, rev).await else {
        return;
    };
    cx.fails(
        &dir.purge(dl.id).await,
        "purge: a list with a kept task in it is Forbidden",
        is_forbidden,
    );
    if fx.advance_clock(40 * 86_400_000).await {
        let lists = dir.lists(Trash::Only).await.unwrap_or_default();
        cx.ensure(
            lists
                .iter()
                .any(|x| x.id == list && matches!(x.trash, Some(Restorable::Held(_)))),
            "lists(Only): past its deadline a list that holds a kept task shows Held, and stays",
        );
    }
}
