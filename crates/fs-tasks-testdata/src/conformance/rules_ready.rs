//! Rules for dependencies: `relate`, `unrelate`, cycles, `Readiness` with and without `dependency.evaluate`.
//!
//! Pattern: Template method (rules as plain checks collected in the report).

use super::cx::{Cx, is_cycle, is_invalid, is_not_found, is_unsupported};
use crate::tasks_v1::{
    Dependency, DependencyKind, Gap, Progress, Readiness, Span, Task, TaskResult,
};

fn dep(other: &Task, kind: DependencyKind) -> Dependency {
    Dependency {
        other_uid: other.uid.clone(),
        kind,
        gap: None,
    }
}

async fn ready_of(
    cx: &Cx<'_>,
    list: crate::tasks_v1::ObjectId,
    id: crate::tasks_v1::ObjectId,
) -> Option<Readiness> {
    cx.summary(list, id).await.map(|s| s.readiness)
}

pub(super) async fn rules(cx: &mut Cx<'_>) {
    let dir = cx.dir;
    let Some(list) = cx.list("ready").await else {
        return;
    };
    let (Some(a), Some(b), Some(c)) = (
        cx.task(list.id, "A").await,
        cx.task(list.id, "B").await,
        cx.task(list.id, "C").await,
    ) else {
        return;
    };
    let gap = Dependency {
        gap: Some(Gap {
            negative: false,
            span: Span {
                days: 1,
                seconds: 0,
            },
        }),
        ..dep(&a, DependencyKind::DependsOn)
    };
    cx.fails(
        &dir.relate(b.id, b.revision, gap).await,
        "relate: a gap on DependsOn is Invalid",
        is_invalid,
    );
    cx.fails(
        &dir.unrelate(b.id, b.revision, &a.uid, DependencyKind::DependsOn)
            .await,
        "unrelate: not related is Invalid",
        is_invalid,
    );
    cx.fails(
        &dir.relate(b.id, b.revision, dep(&b, DependencyKind::DependsOn))
            .await,
        "relate: on itself is a Cycle",
        is_cycle,
    );
    let Ok(b) = dir
        .relate(b.id, b.revision, dep(&a, DependencyKind::DependsOn))
        .await
    else {
        cx.ensure(false, "relate: DependsOn");
        return;
    };
    let cyc = dir
        .relate(a.id, a.revision, dep(&b, DependencyKind::DependsOn))
        .await;
    cx.fails(&cyc, "relate: a cycle is Cycle (with uids)", is_cycle);
    cx.ensure(matches!(&cyc, Err(crate::tasks_v1::TaskError::Cycle(u)) if u.contains(&a.uid) && u.contains(&b.uid)), "relate: Cycle names the uids of the cycle");
    if !cx.caps("dependency.evaluate") {
        without(cx, list.id, &a, &b).await;
        return;
    }
    cx.ensure(
        ready_of(cx, list.id, a.id).await == Some(Readiness::Ready),
        "readiness: no dependency is Ready",
    );
    cx.ensure(
        ready_of(cx, list.id, b.id).await == Some(Readiness::Blocked(vec![a.uid.clone()])),
        "readiness: waits for an open task is Blocked",
    );
    let Some(a) = cx.set_progress(&a, Progress::Completed).await else {
        return;
    };
    cx.ensure(
        ready_of(cx, list.id, b.id).await == Some(Readiness::Ready),
        "readiness: Ready when the predecessor is completed",
    );
    let Some(a) = cx.set_progress(&a, Progress::Cancelled).await else {
        return;
    };
    cx.ensure(
        ready_of(cx, list.id, b.id).await
            == Some(Readiness::DependencyCancelled(vec![a.uid.clone()])),
        "readiness: a cancelled predecessor is DependencyCancelled",
    );
    cx.ensure(
        ready_of(cx, list.id, a.id).await == Some(Readiness::Done),
        "readiness: a finished task is Done",
    );
    let Ok(c) = dir
        .relate(
            c.id,
            c.revision,
            Dependency {
                other_uid: "urn:zzconf:nobody".into(),
                kind: DependencyKind::DependsOn,
                gap: None,
            },
        )
        .await
    else {
        return;
    };
    cx.ensure(
        ready_of(cx, list.id, c.id).await
            == Some(Readiness::Unknown(vec!["urn:zzconf:nobody".into()])),
        "readiness: an unknown uid is Unknown, never guessed",
    );
    let Some(c) = cx.set_progress(&c, Progress::Completed).await else {
        return;
    };
    cx.ensure(
        ready_of(cx, list.id, c.id).await == Some(Readiness::Done),
        "readiness: Done goes before the dependencies",
    );
    successors_and_batch(cx, list.id).await;
}

/// The four time kinds make the holder the predecessor; `readiness` answers in the order asked; `ready` filters.
async fn successors_and_batch(cx: &mut Cx<'_>, list: crate::tasks_v1::ObjectId) {
    let dir = cx.dir;
    let (Some(x), Some(y)) = (cx.task(list, "X").await, cx.task(list, "Y").await) else {
        return;
    };
    let Ok(x) = dir
        .relate(x.id, x.revision, dep(&y, DependencyKind::FinishToStart))
        .await
    else {
        return;
    };
    cx.ensure(
        ready_of(cx, list, y.id).await == Some(Readiness::Blocked(vec![x.uid.clone()])),
        "readiness: FinishToStart makes the holder the predecessor",
    );
    let both = dir.readiness(&[y.id, x.id]).await;
    cx.ensure(
        both.is_ok_and(|v| v.len() == 2 && v[0].0 == y.id && v[1].0 == x.id),
        "readiness: one value per id, in the order asked",
    );
    let missing = dir
        .readiness(&[x.id, crate::ids::object_id(u64::MAX)])
        .await;
    cx.fails(
        &missing,
        "readiness: an unknown id is NotFound for the whole call",
        is_not_found,
    );
    let mut q = cx.query(&[list]);
    q.ready = Some(true);
    let ready = cx.ids(&q).await;
    cx.ensure(
        ready.contains(&x.id) && !ready.contains(&y.id),
        "list: ready = Some(true) only Ready",
    );
    q.ready = Some(false);
    let waiting = cx.ids(&q).await;
    cx.ensure(
        waiting.contains(&y.id) && !waiting.contains(&x.id),
        "list: ready = Some(false) not the Ready ones",
    );
    let Ok(gone) = dir
        .unrelate(x.id, x.revision, &y.uid, DependencyKind::FinishToStart)
        .await
    else {
        return;
    };
    cx.ensure(
        gone.dependencies.is_empty(),
        "unrelate: removes the dependency",
    );
}

/// A provider that cannot evaluate: honest `NotEvaluated`, `Unsupported` for `readiness` and `ready`.
async fn without(cx: &mut Cx<'_>, list: crate::tasks_v1::ObjectId, a: &Task, b: &Task) {
    let dir = cx.dir;
    cx.ensure(
        ready_of(cx, list, a.id).await == Some(Readiness::Ready),
        "readiness (no evaluation): no dependency is Ready",
    );
    cx.ensure(
        ready_of(cx, list, b.id).await == Some(Readiness::NotEvaluated),
        "readiness (no evaluation): an open task with dependencies is NotEvaluated, never Ready",
    );
    let r: TaskResult<_> = dir.readiness(&[a.id]).await;
    cx.fails(&r, "readiness (no evaluation): Unsupported", is_unsupported);
    let mut q = cx.query(&[list]);
    q.ready = Some(true);
    let r = dir.list(&q, None, 10).await;
    cx.fails(
        &r,
        "list (no evaluation): ready = Some(_) is Unsupported, never all ready",
        is_unsupported,
    );
    let done = cx.set_progress(b, Progress::Completed).await;
    cx.ensure(done.is_some(), "setup: complete the dependent task");
    cx.ensure(
        ready_of(cx, list, b.id).await == Some(Readiness::Done),
        "readiness (no evaluation): Done goes before NotEvaluated",
    );
}
