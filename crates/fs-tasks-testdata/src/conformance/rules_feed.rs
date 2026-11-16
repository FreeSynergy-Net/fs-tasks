//! Rules for the change feed, the log, the claim and pages (the labels are `rules_labels`).
//!
//! Pattern: Template method (rules as plain checks collected in the report).

use super::cx::{Cx, is_changed, is_invalid};
use crate::tasks_v1::{HistoryKind, Progress, SeriesScope, TaskPatch};

pub(super) async fn rules(cx: &mut Cx<'_>) {
    let dir = cx.dir;
    let Ok(now) = dir.changes(None, 10).await else {
        cx.ensure(false, "changes: answers");
        return;
    };
    cx.ensure(
        now.items.is_empty() && now.next.is_some(),
        "changes: None = from now on, an empty page still has next",
    );
    let zero = dir.changes(now.next.as_ref(), 0).await;
    cx.ensure(
        zero.is_ok_and(|p| p.items.is_empty() && p.next.is_some()),
        "changes: size 0 is an empty page with next",
    );
    let Some(list) = cx.list("feed").await else {
        return;
    };
    let Some(t) = cx.task(list.id, "Observed").await else {
        return;
    };
    let seen = dir.changes(now.next.as_ref(), 100).await;
    cx.ensure(
        seen.as_ref().is_ok_and(|p| {
            p.items
                .iter()
                .any(|c| c.task == t.id && c.kind == HistoryKind::Create && c.list == list.id)
                && p.next.is_some()
        }),
        "changes: a Create for the new task, next always set",
    );
    let Ok(t) = dir
        .progress(
            t.id,
            t.revision,
            Progress::InProcess,
            crate::tasks_v1::Children::Leave,
            SeriesScope::Whole,
        )
        .await
    else {
        return;
    };
    let Ok(t) = dir.place(t.id, t.revision, None).await else {
        return;
    };
    let Ok(t) = dir
        .update(
            t.id,
            t.revision,
            TaskPatch {
                title: Some("Observed 2".into()),
                ..Default::default()
            },
            SeriesScope::Whole,
        )
        .await
    else {
        return;
    };
    let log = dir
        .history(t.id, None, 100)
        .await
        .map(|p| p.items)
        .unwrap_or_default();
    let kinds: Vec<HistoryKind> = log.iter().map(|e| e.kind).collect();
    cx.ensure(
        kinds
            == [
                HistoryKind::Create,
                HistoryKind::Update,
                HistoryKind::Move,
                HistoryKind::Update,
            ],
        "history: oldest first, one typed entry per write",
    );
    let first = dir.history(t.id, None, 2).await;
    cx.ensure(
        first
            .as_ref()
            .is_ok_and(|p| p.items.len() == 2 && p.next.is_some()),
        "history: pages",
    );
    claim(cx, list.id).await;
}

async fn claim(cx: &mut Cx<'_>, list: crate::tasks_v1::ObjectId) {
    let dir = cx.dir;
    let Some(t) = cx.task(list, "Claimed").await else {
        return;
    };
    let past = dir
        .claim(t.id, t.revision, crate::tasks_v1::Utc::from_millis(0))
        .await;
    cx.fails(
        &past,
        "claim: a deadline in the past is Invalid",
        is_invalid,
    );
    let until = crate::tasks_v1::Utc::from_millis(i64::MAX / 4);
    let Ok(c) = dir.claim(t.id, t.revision, until).await else {
        cx.ensure(false, "claim: takes over");
        return;
    };
    cx.ensure(
        c.claim.as_ref().is_some_and(|x| x.until == until),
        "claim: set until the deadline",
    );
    let Ok(again) = dir.claim(c.id, c.revision, until).await else {
        cx.ensure(false, "claim: the own claim may be renewed");
        return;
    };
    cx.fails(
        &dir.claim(c.id, c.revision, until).await,
        "claim: compare-and-set",
        is_changed,
    );
    let Ok(freed) = dir.release(again.id, again.revision).await else {
        cx.ensure(false, "release: gives back");
        return;
    };
    cx.ensure(freed.claim.is_none(), "release: the claim is gone");
}
