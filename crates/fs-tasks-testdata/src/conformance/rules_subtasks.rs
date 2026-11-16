//! Rules for subtasks: parent cycles, the counts on the card, and what `progress` does with open subtasks.
//!
//! Pattern: Template method (rules as plain checks collected in the report).

use super::cx::{Cx, is_cycle, is_invalid};
use crate::tasks_v1::{
    Children, ObjectId, Progress, SUBTASK_DEPTH_MAX, SeriesScope, TaskError, TaskPatch,
};

/// Subtasks: parent cycles, and what `progress` does with open subtasks.
pub(super) async fn rules(cx: &mut Cx<'_>, list: crate::tasks_v1::ObjectId) {
    let dir = cx.dir;
    let Some(parent) = cx.task(list, "Parent").await else {
        return;
    };
    let kid = |p| TaskPatch {
        parent: Some(Some(p)),
        ..Default::default()
    };
    let Some(k1) = cx.task_with(list, "Kid 1", kid(parent.id)).await else {
        return;
    };
    let Some(k2) = cx.task_with(list, "Kid 2", kid(parent.id)).await else {
        return;
    };
    let cyc = dir
        .update(
            parent.id,
            parent.revision,
            TaskPatch {
                parent: Some(Some(k1.id)),
                ..Default::default()
            },
            SeriesScope::Whole,
        )
        .await;
    cx.fails(
        &cyc,
        "update: a parent that closes a cycle is Cycle",
        is_cycle,
    );
    let s = cx.summary(list, parent.id).await;
    cx.ensure(
        s.is_some_and(|s| s.subtasks_total == 2 && s.subtasks_open == 2),
        "list: subtask counts",
    );
    if !cx.needs("subtasks and progress", &["task.progress"]) {
        return;
    }

    let refused = dir
        .progress(
            parent.id,
            parent.revision,
            Progress::Completed,
            Children::Refuse,
            SeriesScope::Whole,
        )
        .await;
    let listed = matches!(&refused, Err(TaskError::OpenSubtasks(ids)) if ids.len() == 2 && ids.contains(&k1.id));
    cx.ensure(
        listed,
        "progress: Refuse with open subtasks is OpenSubtasks with their ids",
    );
    cx.ensure(
        cx.fresh(parent.id)
            .await
            .is_some_and(|p| p.progress == Progress::NeedsAction && p.revision == parent.revision),
        "progress: OpenSubtasks writes nothing",
    );
    let left = dir
        .progress(
            parent.id,
            parent.revision,
            Progress::Completed,
            Children::Leave,
            SeriesScope::Whole,
        )
        .await;
    cx.ensure(left.is_ok(), "progress: Leave completes the parent");
    cx.ensure(
        cx.fresh(k1.id)
            .await
            .is_some_and(|k| k.progress == Progress::NeedsAction),
        "progress: Leave keeps the subtasks",
    );
    let Ok(parent) = left else { return };
    let reopened = dir
        .progress(
            parent.id,
            parent.revision,
            Progress::NeedsAction,
            Children::Refuse,
            SeriesScope::Whole,
        )
        .await;
    cx.ensure(
        reopened
            .as_ref()
            .is_ok_and(|p| p.progress == Progress::NeedsAction),
        "progress: reopens",
    );
    let Ok(parent) = reopened else { return };
    let same = dir
        .progress(
            parent.id,
            parent.revision,
            Progress::Cancelled,
            Children::Same,
            SeriesScope::Whole,
        )
        .await;
    cx.ensure(same.is_ok(), "progress: Same cancels the parent");
    cx.ensure(
        cx.fresh(k2.id)
            .await
            .is_some_and(|k| k.progress == Progress::Cancelled),
        "progress: Same gives the open subtasks the same state",
    );
}

/// A chain of `SUBTASK_DEPTH_MAX` levels fits; a deeper one is `Invalid` (`parent`), also by moving a whole subtree.
pub(super) async fn depth(cx: &mut Cx<'_>, list: ObjectId) {
    let dir = cx.dir;
    let under = |p: ObjectId| TaskPatch {
        parent: Some(Some(p)),
        ..TaskPatch::default()
    };
    let Some(top) = cx.task(list, "level 1").await else {
        return;
    };
    let mut at = top.id;
    for level in 2..=SUBTASK_DEPTH_MAX {
        let made = dir
            .create(super::cx::draft(list, &format!("level {level}"), under(at)))
            .await;
        let Ok(t) = made else {
            cx.ensure(
                false,
                "create: a subtask chain up to SUBTASK_DEPTH_MAX levels",
            );
            return;
        };
        at = t.id;
    }
    let deeper = dir
        .create(super::cx::draft(list, "too deep", under(at)))
        .await;
    cx.fails(
        &deeper,
        "create: a subtask on level SUBTASK_DEPTH_MAX + 1 is Invalid",
        is_invalid,
    );
    let (Some(lone), Some(lone_kid)) =
        (cx.task(list, "lone").await, cx.task(list, "lone kid").await)
    else {
        return;
    };
    let Ok(lone_kid) = dir
        .update(
            lone_kid.id,
            lone_kid.revision,
            under(lone.id),
            SeriesScope::Whole,
        )
        .await
    else {
        return;
    };
    let moved = dir
        .update(lone.id, lone.revision, under(at), SeriesScope::Whole)
        .await;
    cx.fails(
        &moved,
        "update: moving a task with subtasks so the tree gets deeper than SUBTASK_DEPTH_MAX is Invalid (the whole subtree is checked)",
        is_invalid,
    );
    let _ = lone_kid;
}

/// `restore` checks the depth of the tree it brings back: the parent moved deeper meanwhile, so the task and the
/// subtask that went with it no longer fit (`Invalid`, `parent`), and nothing was restored.
pub(super) async fn restore_depth(cx: &mut Cx<'_>, list: ObjectId) {
    let dir = cx.dir;
    let under = |p: ObjectId| TaskPatch {
        parent: Some(Some(p)),
        ..TaskPatch::default()
    };
    let (Some(p0), Some(mut at)) = (
        cx.task(list, "restore parent").await,
        cx.task(list, "chain 1").await,
    ) else {
        return;
    };
    let Some(x) = cx.task_with(list, "restore x", under(p0.id)).await else {
        return;
    };
    if cx.task_with(list, "restore y", under(x.id)).await.is_none() {
        return;
    }
    let Ok(gone) = dir.delete(x.id, x.revision, SeriesScope::Whole).await else {
        cx.ensure(false, "restore depth setup: delete the subtree");
        return;
    };
    for level in 2..SUBTASK_DEPTH_MAX {
        let made = dir
            .create(super::cx::draft(
                list,
                &format!("chain {level}"),
                under(at.id),
            ))
            .await;
        let Ok(t) = made else {
            cx.ensure(false, "restore depth setup: the chain of levels");
            return;
        };
        at = t;
    }
    let Some(p0) = cx.fresh(p0.id).await else {
        cx.ensure(false, "restore depth setup: read the parent again");
        return;
    };
    let moved = dir
        .update(p0.id, p0.revision, under(at.id), SeriesScope::Whole)
        .await;
    cx.ensure(
        moved.is_ok(),
        "restore depth setup: the parent moves to the last level",
    );
    let back = dir.restore(gone.id, SeriesScope::Whole).await;
    cx.fails(
        &back,
        "restore: a task whose tree would be deeper than SUBTASK_DEPTH_MAX is Invalid",
        is_invalid,
    );
}
