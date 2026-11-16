//! Rules for the trash of a task: delete with subtasks, restore, the three trash views, the deadline.
//!
//! Pattern: Template method (rules as plain checks collected in the report).

use super::ConformanceFixture;
use super::cx::{Cx, is_changed, is_invalid, is_not_found};
use crate::tasks_v1::{Restorable, SeriesScope, TaskPatch, Trash};

pub(super) async fn rules(cx: &mut Cx<'_>) {
    let dir = cx.dir;
    let Some(list) = cx.list("trash").await else {
        return;
    };
    let Some(p) = cx.task(list.id, "Parent").await else {
        return;
    };
    let kid = TaskPatch {
        parent: Some(Some(p.id)),
        ..Default::default()
    };
    let (Some(k), Some(keep)) = (
        cx.task_with(list.id, "Kid", kid).await,
        cx.task(list.id, "Keep").await,
    ) else {
        return;
    };
    let stale = dir
        .delete(
            p.id,
            crate::tasks_v1::Revision(p.revision.0 + 5),
            SeriesScope::Whole,
        )
        .await;
    cx.fails(&stale, "delete: a stale revision is Changed", is_changed);
    let Ok(d) = dir.delete(p.id, p.revision, SeriesScope::Whole).await else {
        cx.ensure(false, "delete: answers");
        return;
    };
    cx.ensure(
        d.with_subtasks == [k.id] && d.scope == SeriesScope::Whole,
        "delete: takes the subtasks along",
    );
    cx.ensure(
        d.restorable.is_some(),
        "delete: a deadline (there is no unlimited trash)",
    );
    cx.ensure(
        dir.get(p.id).await.ok().flatten().is_none(),
        "get: a deleted task is None",
    );
    let mut q = cx.query(&[list.id]);
    cx.ensure(
        cx.ids(&q).await == [keep.id],
        "list(Exclude): only the living",
    );
    q.trash = Trash::Include;
    cx.ensure(
        cx.all(&q).await.len() == 3,
        "list(Include): living and deleted mixed",
    );
    q.trash = Trash::Only;
    let only = cx.all(&q).await;
    cx.ensure(
        only.len() == 2 && only.iter().all(|s| s.trash == d.restorable),
        "list(Only): the deleted, with their deadline",
    );
    let alone = dir.restore(k.id, SeriesScope::Whole).await;
    cx.fails(
        &alone,
        "restore: a subtask deleted with its parent only comes back with it",
        is_invalid,
    );
    let Ok(back) = dir.restore(p.id, SeriesScope::Whole).await else {
        cx.ensure(false, "restore: answers");
        return;
    };
    cx.ensure(
        back.id == p.id && cx.fresh(k.id).await.is_some(),
        "restore: the task and its subtasks are alive again",
    );
    let alive = dir.restore(p.id, SeriesScope::Whole).await;
    cx.fails(&alive, "restore: a living task is Invalid", is_invalid);
    let unknown = dir
        .restore(crate::ids::object_id(u64::MAX), SeriesScope::Whole)
        .await;
    cx.fails(
        &unknown,
        "restore: an unknown task is NotFound",
        is_not_found,
    );
}

/// After the deadline everything is gone for good.
pub(super) async fn deadline(fx: &dyn ConformanceFixture, cx: &mut Cx<'_>) {
    let dir = cx.dir;
    let Some(list) = cx.list("deadline").await else {
        return;
    };
    let Some(t) = cx.task(list.id, "Soon gone").await else {
        return;
    };
    let Ok(d) = dir.delete(t.id, t.revision, SeriesScope::Whole).await else {
        return;
    };
    if d.restorable.is_none() {
        return;
    }
    if !fx.advance_clock(31 * 86_400_000).await {
        cx.skip("restore: after the deadline (fixture cannot move the clock)");
        return;
    }
    let r = dir.restore(t.id, SeriesScope::Whole).await;
    cx.fails(
        &r,
        "restore: after the deadline the task is gone (NotFound)",
        is_not_found,
    );
    let mut q = cx.query(&[list.id]);
    q.trash = Trash::Only;
    cx.ensure(
        cx.all(&q).await.is_empty(),
        "list(Only): nothing after the deadline",
    );
}

const HOUR: i64 = 3_600_000;

/// The deadline is worked out at the time of the delete (its time plus the trash period), not a fixed date: two deletes
/// an hour apart have deadlines an hour apart (a provider that ticks its clock on every write is a few ms off, the fake
/// a few seconds; a minute either way is fine), for a task and for a
/// list. Opus review K0c-3: a provider that has no `restorable_until` of its own computes this itself.
pub(super) async fn moves(fx: &dyn ConformanceFixture, cx: &mut Cx<'_>) {
    let dir = cx.dir;
    let Some(list) = cx.list("deadline moves").await else {
        return;
    };
    let (Some(a), Some(b)) = (
        cx.task(list.id, "first deleted").await,
        cx.task(list.id, "second deleted").await,
    ) else {
        return;
    };
    let (Some(l1), Some(l2)) = (
        cx.list("deadline list one").await,
        cx.list("deadline list two").await,
    ) else {
        return;
    };
    let first = dir.delete(a.id, a.revision, SeriesScope::Whole).await;
    let first_list = dir.delete_list(l1.id, l1.revision).await;
    if !fx.advance_clock(HOUR).await {
        cx.skip("delete: the deadline moves with the clock (the fixture cannot move the clock)");
        return;
    }
    let second = dir.delete(b.id, b.revision, SeriesScope::Whole).await;
    let second_list = dir.delete_list(l2.id, l2.revision).await;
    let (Ok(first), Ok(second), Ok(first_list), Ok(second_list)) =
        (first, second, first_list, second_list)
    else {
        cx.ensure(false, "deadline setup: delete two tasks and two lists");
        return;
    };
    for (one, two, what) in [
        (first.restorable, second.restorable, "a task"),
        (first_list.restorable, second_list.restorable, "a list"),
    ] {
        let (Some(Restorable::Until(x)), Some(Restorable::Until(y))) = (one, two) else {
            // Not `Until` here means the provider did not work out a deadline from the time of the delete: a failure
            // for a full provider (Opus review K1-1, A5); a staged one names the skip.
            let rule = format!(
                "delete: the deadline of {what} is `Until` (the time of the delete plus the trash period)"
            );
            if cx.partial {
                cx.skip(&rule);
            } else {
                cx.ensure(false, &rule);
            }
            continue;
        };
        let gap = y.millis() - x.millis();
        cx.ensure(
            (HOUR - 60_000..HOUR + 60_000).contains(&gap),
            &format!(
                "delete: the deadline of {what} is the time of the delete plus the trash period (an hour later, an hour later)"
            ),
        );
    }
}
