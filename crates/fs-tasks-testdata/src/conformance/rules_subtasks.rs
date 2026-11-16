//! Rules for subtasks: parent cycles, the counts on the card, and what `progress` does with open subtasks.
//!
//! Pattern: Template method (rules as plain checks collected in the report).

use super::cx::{Cx, is_cycle};
use crate::tasks_v1::{Children, Progress, SeriesScope, TaskError, TaskPatch};

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
