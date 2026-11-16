//! Rules for the order of the checks of the claim: `Forbidden` before `Claimed` (an outsider is not told who holds a
//! task), and `release` of an expired claim by somebody who may not claim is `Forbidden` as well.
//!
//! Pattern: Template method (rules as plain checks collected in the report).

use super::ConformanceFixture;
use super::cx::{Cx, is_forbidden};
use super::rules_claim::{assigned, far};
use crate::tasks_v1::TaskError;

pub(super) async fn rules(fx: &dyn ConformanceFixture, cx: &mut Cx<'_>) {
    if !cx.caps("task.claim") {
        return;
    }
    let Some(colleague) = fx.colleague().await else {
        cx.skip("claim: Forbidden before Claimed (the fixture has no colleague)");
        return;
    };
    let Some(list) = cx.list("claim order").await else {
        return;
    };
    let dir = cx.dir;
    let Some(t) = assigned(
        cx,
        list.id,
        "only the colleague",
        std::slice::from_ref(&colleague),
    )
    .await
    else {
        return;
    };
    if !fx.act_as(Some(colleague.clone())).await {
        cx.skip("claim: Forbidden before Claimed (the fixture cannot act as the colleague)");
        return;
    }
    let theirs = dir.claim(t.id, t.revision, far(&t)).await;
    fx.act_as(None).await;
    let Ok(theirs) = theirs else {
        cx.skip("claim: Forbidden before Claimed (the colleague could not claim)");
        return;
    };
    let me_claims = dir.claim(t.id, theirs.revision, far(&t)).await;
    cx.ensure(
        matches!(&me_claims, Err(e) if is_forbidden(e) && !matches!(e, TaskError::Claimed(_))),
        "claim: Forbidden comes before Claimed (an outsider is not told who holds the task)",
    );
    let me_releases = dir.release(t.id, theirs.revision).await;
    cx.fails(
        &me_releases,
        "release: a claim that holds, by somebody who may not claim, is Forbidden",
        is_forbidden,
    );
    if !fx.advance_clock(500 * 86_400_000).await {
        cx.skip("release: an expired claim by an outsider is Forbidden (the fixture cannot move the clock)");
        return;
    }
    let late = dir.release(t.id, theirs.revision).await;
    cx.fails(
        &late,
        "release: an expired claim is released only by somebody who may claim, an outsider gets Forbidden",
        is_forbidden,
    );
}
