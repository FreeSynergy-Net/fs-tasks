//! Rules for the hand order that need `task.place`: neighbours, the bottom, and the one-time spread by placing. The
//! rules of the top (new tasks, `create` only) are in `rules_rank_top`.
//!
//! Pattern: Template method (rules as plain checks collected in the report).

use super::cx::{Cx, is_invalid};
use crate::tasks_v1::{HistoryKind, ObjectId, Rank, TaskPatch};

/// The ids of a list in rank order.
pub(super) async fn order(cx: &Cx<'_>, list: ObjectId) -> Vec<ObjectId> {
    cx.ids(&cx.query(&[list])).await
}

/// The longest rank an edge may reach after `n` insertions at the same edge: about 25 per character (the step of
/// `Rank::between`), plus two. Halving at the edge would need 4 to 5 per character and fail this.
pub(super) fn edge_len(n: usize) -> usize {
    n / 25 + 2
}

/// Ranks in the listing are strictly ascending and at most `max` characters long.
pub(super) async fn ascending(cx: &Cx<'_>, list: ObjectId, max: usize) -> bool {
    let ranks: Vec<Rank> = cx
        .all(&cx.query(&[list]))
        .await
        .into_iter()
        .map(|s| s.rank)
        .collect();
    ranks.windows(2).all(|w| w[0] < w[1]) && ranks.iter().all(|r| r.as_str().len() <= max)
}

pub(super) async fn rules(cx: &mut Cx<'_>) {
    let dir = cx.dir;
    let Some(list) = cx.list("rank").await else {
        return;
    };
    let (Some(a), Some(b), Some(c)) = (
        cx.task(list.id, "a").await,
        cx.task(list.id, "b").await,
        cx.task(list.id, "c").await,
    ) else {
        return;
    };
    let moved = dir.place(a.id, a.revision, None).await;
    cx.ensure(
        moved.is_ok() && order(cx, list.id).await == [a.id, c.id, b.id],
        "place: None = ganz oben",
    );
    let Some(b) = cx.fresh(b.id).await else {
        return;
    };
    let moved = dir
        .place(a.id, moved.map_or(a.revision, |m| m.revision), Some(b.id))
        .await;
    cx.ensure(
        moved.is_ok() && order(cx, list.id).await == [c.id, b.id, a.id],
        "place: directly after the named task",
    );
    cx.ensure(
        cx.fresh(b.id)
            .await
            .is_some_and(|x| x.revision == b.revision),
        "place: writes only the moved task (no revision of the neighbours)",
    );
    let Some(a) = cx.fresh(a.id).await else {
        return;
    };
    let outside = cx.list("rank other").await;
    if let Some(x) = match outside {
        Some(o) => cx.task(o.id, "x").await,
        None => None,
    } {
        let r = dir.place(a.id, a.revision, Some(x.id)).await;
        cx.fails(
            &r,
            "place: after a task outside the rank sequence is Invalid",
            is_invalid,
        );
    }
    let r = dir.place(a.id, a.revision, Some(a.id)).await;
    cx.fails(&r, "place: after itself is Invalid", is_invalid);
    appends(cx).await;
    edges(cx).await;
    spread(cx).await;
}

/// 1000 appends at the bottom (`place`): short ranks, nobody else is written (the top is `rules_rank_top`).
async fn appends(cx: &mut Cx<'_>) {
    let dir = cx.dir;
    let Some(list) = cx.list("rank bottom").await else {
        return;
    };
    let Some(anchor) = cx.task(list.id, "anchor").await else {
        return;
    };
    let mut last = anchor.id;
    for i in 0..1000 {
        let Ok(t) = dir
            .create(super::cx::draft(
                list.id,
                &format!("b{i}"),
                TaskPatch::default(),
            ))
            .await
        else {
            cx.ensure(false, "create: 1000 appends at the bottom");
            return;
        };
        let Ok(p) = dir.place(t.id, t.revision, Some(last)).await else {
            cx.ensure(false, "place: 1000 appends at the bottom");
            return;
        };
        last = p.id;
    }
    cx.ensure(
        ascending(cx, list.id, edge_len(1000)).await,
        "rank: 1000 appends at the bottom stay ascending and at most 1000/25 + 2 characters long",
    );
    cx.ensure(
        order(cx, list.id).await.last() == Some(&last),
        "rank: the last appended task is last",
    );
}

/// `Cx::edge` insertions (2000; 6000 with the `heavy` switch of the night run) at the bottom (each one placed after the
/// last): ranks within the edge bound, a `Move` for each placed task. The top edge is `rules_rank_top` (no `place`).
async fn edges(cx: &mut Cx<'_>) {
    let n = cx.edge;
    let dir = cx.dir;
    let Some(list) = cx.list("rank edge bottom").await else {
        return;
    };
    let Some(anchor) = cx.task(list.id, "anchor").await else {
        return;
    };
    let start = start_of(cx).await;
    let mut last = anchor.id;
    for i in 0..n {
        let draft = super::cx::draft(list.id, &format!("e{i}"), TaskPatch::default());
        let Ok(t) = dir.create(draft).await else {
            cx.ensure(false, &format!("create: {n} insertions at the edge"));
            return;
        };
        let Ok(p) = dir.place(t.id, t.revision, Some(last)).await else {
            cx.ensure(false, &format!("place: {n} insertions at the bottom"));
            return;
        };
        last = p.id;
    }
    cx.ensure(
        ascending(cx, list.id, edge_len(n)).await,
        &format!("rank: {n} insertions at the bottom stay ascending and at most {n}/25 + 2 characters long"),
    );
    if let Some(start) = start {
        let moves = cx.changes_since(start.next.as_ref()).await;
        let moved = moves.iter().filter(|c| c.kind == HistoryKind::Move).count();
        cx.ensure(
            moved == n,
            &format!("rank: {n} insertions at the bottom spread nothing (a Move only for each placed task)"),
        );
    } else {
        cx.skip("rank: no Move for the placed tasks (the provider has no task.changes)");
    }
}

/// Always into the same gap, alternately above the lower and below the upper neighbour, until the rank runs out: the provider spreads once, in the same transaction.
async fn spread(cx: &mut Cx<'_>) {
    let dir = cx.dir;
    let Some(list) = cx.list("rank spread").await else {
        return;
    };
    let (Some(low), Some(high)) = (
        cx.task(list.id, "low").await,
        cx.task(list.id, "high").await,
    ) else {
        return;
    };
    let start = start_of(cx).await;
    let mut moved_without_changed = true;
    let mut lower = high.id;
    for i in 0..2500 {
        let Ok(t) = dir
            .create(super::cx::draft(
                list.id,
                &format!("g{i}"),
                TaskPatch::default(),
            ))
            .await
        else {
            moved_without_changed = false;
            break;
        };
        match dir.place(t.id, t.revision, Some(lower)).await {
            Ok(p) if p.revision.0 == t.revision.0 + 1 => {
                if i % 2 == 0 {
                    lower = p.id;
                }
            }
            _ => {
                moved_without_changed = false;
                break;
            }
        }
        // The spread shows as a new revision of a neighbour: stop there (the same gap runs out after about 250 steps).
        if i % 16 == 15 && spread_seen(cx, &low, &high).await {
            break;
        }
    }
    cx.ensure(
        moved_without_changed,
        "place: the spread is in the same transaction, no Changed for the caller",
    );
    cx.ensure(
        ascending(cx, list.id, Rank::MAX_LEN).await,
        "rank: after the spread the order is ascending",
    );
    let (Some(l), Some(h)) = (cx.fresh(low.id).await, cx.fresh(high.id).await) else {
        return;
    };
    cx.ensure(
        l.revision > low.revision || h.revision > high.revision,
        "rank: the spread gives the tasks whose rank changed a new revision",
    );
    if let Some(start) = start {
        let moves = cx.changes_since(start.next.as_ref()).await;
        let by_spread = moves
            .iter()
            .filter(|c| c.kind == HistoryKind::Move && (c.task == low.id || c.task == high.id))
            .count();
        cx.ensure(
            by_spread > 0,
            "changes: a Move for each task the spread rewrote",
        );
    } else {
        cx.skip("rank: a Move for each task the spread rewrote (the provider has no task.changes)");
    }
    if cx.caps("task.history") {
        let log = dir
            .history(low.id, None, 100)
            .await
            .map(|p| p.items)
            .unwrap_or_default();
        cx.ensure(
            !log.iter().any(|e| e.kind == HistoryKind::Move),
            "history: the spread writes no Move into the log of a task nobody moved",
        );
    } else {
        cx.skip("rank: the spread writes no Move into a log (the provider has no task.history)");
    }
}

/// The start of the feed, when the provider has one.
async fn start_of(cx: &Cx<'_>) -> Option<crate::tasks_v1::Page<crate::tasks_v1::Change>> {
    if cx.caps("task.changes") {
        cx.dir.changes(None, 0).await.ok()
    } else {
        None
    }
}

/// Whether a task of the gap has a newer revision than it was created with.
async fn spread_seen(
    cx: &Cx<'_>,
    low: &crate::tasks_v1::Task,
    high: &crate::tasks_v1::Task,
) -> bool {
    let (l, h) = (cx.fresh(low.id).await, cx.fresh(high.id).await);
    l.is_some_and(|l| l.revision > low.revision) || h.is_some_and(|h| h.revision > high.revision)
}
