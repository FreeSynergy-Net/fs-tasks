//! Rules for `labels`: alphabetical, counted once per task, the prefix, pages, only living tasks (they stand apart from
//! the feed, a provider without `task.changes` runs them too).
//!
//! Pattern: Template method (rules as plain checks collected in the report).

use super::cx::Cx;
use crate::tasks_v1::{SeriesScope, TaskPatch};

pub(super) async fn rules(cx: &mut Cx<'_>) {
    let dir = cx.dir;
    let Some(list) = cx.list("labels").await else {
        return;
    };
    let list = list.id;
    let with = |l: &[&str]| TaskPatch {
        labels: Some(l.iter().map(|x| (*x).to_owned()).collect()),
        ..Default::default()
    };
    let (Some(_), Some(_), Some(gone)) = (
        cx.task_with(list, "L1", with(&["zz-b", "zz-a"])).await,
        cx.task_with(list, "L2", with(&["zz-b", "zz-b", "yy"]))
            .await,
        cx.task_with(list, "L3", with(&["zz-c"])).await,
    ) else {
        return;
    };
    let all = dir
        .labels(&[list], Some("zz-"), None, 10)
        .await
        .map(|p| p.items)
        .unwrap_or_default();
    let names: Vec<(&str, u32)> = all.iter().map(|l| (l.label.as_str(), l.count)).collect();
    cx.ensure(
        names == [("zz-a", 1), ("zz-b", 2), ("zz-c", 1)],
        "labels: alphabetical, counted once per task, prefix applied",
    );
    let page = dir.labels(&[list], None, None, 2).await;
    cx.ensure(
        page.is_ok_and(|p| p.items.len() == 2 && p.next.is_some()),
        "labels: pages",
    );
    let _ = dir.delete(gone.id, gone.revision, SeriesScope::Whole).await;
    let left = dir
        .labels(&[list], Some("zz-c"), None, 10)
        .await
        .map(|p| p.items);
    cx.ensure(
        left.is_ok_and(|v| v.is_empty()),
        "labels: only living tasks count",
    );
}
