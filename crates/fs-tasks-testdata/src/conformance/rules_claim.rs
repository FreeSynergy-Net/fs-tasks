//! Rules for the claim: who may take a task, the order of the checks, `release`, `claim_next`; the rules that need
//! groups, a second human or an AI are in `groups_rules`.
//!
//! Pattern: Template method (rules as plain checks collected in the report).

use super::cx::{Cx, draft, is_changed, is_forbidden, is_invalid, is_not_found};
use crate::tasks_v1::{
    HistoryKind, ObjectId, PersonRef, Progress, Task, TaskDraft, TaskPatch, TaskQuery, Utc,
};

pub(super) fn far(t: &Task) -> Utc {
    Utc::from_millis(t.updated.millis() + 400 * 86_400_000)
}

pub(super) async fn assigned(
    cx: &mut Cx<'_>,
    list: ObjectId,
    title: &str,
    who: &[PersonRef],
) -> Option<Task> {
    let made = cx
        .dir
        .create(TaskDraft {
            assignees: who.to_vec(),
            ..draft(list, title, TaskPatch::default())
        })
        .await;
    cx.ensure(made.is_ok(), &format!("setup: create {title}"));
    made.ok()
}

pub(super) async fn rules(cx: &mut Cx<'_>) {
    if !cx.caps("task.claim") {
        cx.skip("claim: the provider has no task.claim");
        return;
    }
    let Some(list) = cx.list("claim").await else {
        return;
    };
    let dir = cx.dir;
    let Ok(me) = dir.asker().await else { return };
    let stranger = crate::ids::person(4242);
    let (Some(free), Some(mine), Some(theirs)) = (
        cx.task(list.id, "free").await,
        assigned(cx, list.id, "mine", std::slice::from_ref(&me)).await,
        assigned(cx, list.id, "theirs", std::slice::from_ref(&stranger)).await,
    ) else {
        return;
    };
    let until = far(&free);
    let Ok(held) = dir.claim(free.id, free.revision, until).await else {
        cx.ensure(
            false,
            "claim: a task nobody is assigned to, with write access",
        );
        return;
    };
    cx.ensure(
        held.claim
            .as_ref()
            .is_some_and(|c| c.by == me && c.until == until)
            && held.revision > free.revision,
        "claim: the claim names the asking person and the time; the revision goes up",
    );
    let sum = cx.summary(list.id, free.id).await;
    cx.ensure(
        sum.is_some_and(|s| s.claimed_by.as_ref() == Some(&me)),
        "list: the card says who holds the claim",
    );
    let renewed = dir.claim(held.id, held.revision, until).await;
    cx.ensure(renewed.is_ok(), "claim: the own claim may be renewed");
    cx.ensure(
        dir.claim(mine.id, mine.revision, until).await.is_ok(),
        "claim: a task assigned to the asking person",
    );
    let other = dir.claim(theirs.id, theirs.revision, until).await;
    cx.fails(
        &other,
        "claim: a task assigned only to someone else is Forbidden",
        is_forbidden,
    );
    let missing = dir
        .claim(
            crate::ids::object_id(u64::MAX),
            crate::tasks_v1::Revision(1),
            until,
        )
        .await;
    cx.fails(&missing, "claim: an unknown task is NotFound", is_not_found);
    let past = dir.claim(free.id, free.revision, Utc::from_millis(1)).await;
    cx.fails(
        &past,
        "claim: a time in the past is Invalid (before the revision check)",
        |e| is_invalid(e) || is_changed(e),
    );
    ends(cx, list.id).await;
    next(cx, list.id).await;
}

/// The claim ends with `release` and with a finished state; the log says `Join` and `Leave`.
async fn ends(cx: &mut Cx<'_>, list: ObjectId) {
    let dir = cx.dir;
    let Some(t) = cx.task(list, "ends").await else {
        return;
    };
    let Ok(c) = dir.claim(t.id, t.revision, far(&t)).await else {
        return;
    };
    let stale = dir.release(c.id, t.revision).await;
    cx.fails(&stale, "release: a stale revision is Changed", is_changed);
    let Ok(r) = dir.release(c.id, c.revision).await else {
        cx.ensure(false, "release: the holder gives it back");
        return;
    };
    cx.ensure(r.claim.is_none(), "release: the claim is gone");
    let again = dir.release(r.id, r.revision).await;
    cx.ensure(
        again.is_ok_and(|a| a.revision == r.revision),
        "release: nothing to give back changes nothing",
    );
    if cx.caps("task.history") {
        let kinds: Vec<HistoryKind> = dir
            .history(t.id, None, 100)
            .await
            .map(|p| p.items.into_iter().map(|e| e.kind).collect())
            .unwrap_or_default();
        cx.ensure(
            kinds.contains(&HistoryKind::Join) && kinds.contains(&HistoryKind::Leave),
            "history: Join when taken, Leave when given back",
        );
    }
    let Ok(c2) = dir.claim(r.id, r.revision, far(&r)).await else {
        return;
    };
    let Ok(done) = cx.set_progress(&c2, Progress::Completed).await.ok_or(()) else {
        return;
    };
    cx.ensure(
        done.claim.is_none(),
        "progress: a finished task has no claim any more",
    );
    let late = dir.claim(done.id, done.revision, far(&done)).await;
    cx.fails(&late, "claim: a finished task is Invalid", is_invalid);
}

/// `claim_next`: the first task that is open, free and claimable, in the order of the query; one transaction.
async fn next(cx: &mut Cx<'_>, list: ObjectId) {
    let dir = cx.dir;
    let Ok(me) = dir.asker().await else { return };
    let Some(l) = cx.list("claim next").await else {
        return;
    };
    let stranger = crate::ids::person(4242);
    let (Some(a), Some(b), Some(_theirs)) = (
        cx.task(l.id, "a free").await,
        assigned(cx, l.id, "b mine", std::slice::from_ref(&me)).await,
        assigned(cx, l.id, "c theirs", std::slice::from_ref(&stranger)).await,
    ) else {
        return;
    };
    let q = cx.query(&[l.id]);
    let until = far(&a);
    let first = dir.claim_next(&q, until).await;
    cx.ensure(
        matches!(&first, Ok(Some(t)) if t.id == b.id && t.claim.as_ref().is_some_and(|k| k.by == me)),
        "claim_next: the first claimable task in the order of the query (not the one assigned to someone else)",
    );
    if let Ok(Some(t)) = first {
        let _ = cx.set_progress(&t, Progress::Completed).await;
    }
    let second = dir.claim_next(&q, until).await;
    cx.ensure(
        matches!(&second, Ok(Some(t)) if t.id == a.id),
        "claim_next: a finished task is skipped, the next one is taken",
    );
    if let Ok(Some(t)) = second {
        let _ = cx.set_progress(&t, Progress::Completed).await;
    }
    cx.ensure(
        matches!(dir.claim_next(&q, until).await, Ok(None)),
        "claim_next: None when nothing is claimable (only the task of someone else is left)",
    );
    invalid_queries(cx, &q, until).await;
    blocked(cx, list).await;
}

async fn invalid_queries(cx: &mut Cx<'_>, q: &TaskQuery, until: Utc) {
    let dir = cx.dir;
    let ready = TaskQuery {
        ready: Some(true),
        ..q.clone()
    };
    cx.fails(
        &dir.claim_next(&ready, until).await,
        "claim_next: a query with ready is Invalid",
        |e| is_invalid(e) || super::cx::is_unsupported(e),
    );
    let trash = TaskQuery {
        trash: crate::tasks_v1::Trash::Only,
        ..q.clone()
    };
    cx.fails(
        &dir.claim_next(&trash, until).await,
        "claim_next: a query with trash other than Exclude is Invalid",
        is_invalid,
    );
    cx.fails(
        &dir.claim_next(q, Utc::from_millis(1)).await,
        "claim_next: a time in the past is Invalid",
        is_invalid,
    );
}

/// A task that waits for another is never taken by `claim_next` (also without `dependency.evaluate`).
async fn blocked(cx: &mut Cx<'_>, _list: ObjectId) {
    let dir = cx.dir;
    let Some(l) = cx.list("claim blocked").await else {
        return;
    };
    let Some(y) = cx.task(l.id, "y first").await else {
        return;
    };
    let Some(x) = cx.task(l.id, "x waits").await else {
        return;
    };
    let dep = crate::tasks_v1::Dependency {
        other_uid: y.uid.clone(),
        kind: crate::tasks_v1::DependencyKind::DependsOn,
        gap: None,
    };
    if dir.relate(x.id, x.revision, dep).await.is_err() {
        cx.skip("claim_next: skips a waiting task (the provider has no relate)");
        return;
    }
    let got = dir.claim_next(&cx.query(&[l.id]), far(&y)).await;
    cx.ensure(
        matches!(&got, Ok(Some(t)) if t.id == y.id),
        "claim_next: a task with dependencies that is not ready is skipped, never taken",
    );
}
