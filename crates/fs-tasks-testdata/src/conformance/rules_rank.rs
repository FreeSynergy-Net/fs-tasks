//! Rules for the hand order (`Rank`): top and bottom, neighbours, 1000 appends, and the one-time spread.
//!
//! Pattern: Template method (rules as plain checks collected in the report).

use super::cx::{Cx, is_invalid};
use crate::tasks_v1::{HistoryKind, ObjectId, Rank, TaskPatch};

/// The ids of a list in rank order.
async fn order(cx: &Cx<'_>, list: ObjectId) -> Vec<ObjectId> {
    cx.ids(&cx.query(&[list])).await
}

/// Ranks in the listing are strictly ascending and short enough.
async fn ascending(cx: &Cx<'_>, list: ObjectId) -> bool {
    let ranks: Vec<Rank> = cx
        .all(&cx.query(&[list]))
        .await
        .into_iter()
        .map(|s| s.rank)
        .collect();
    ranks.windows(2).all(|w| w[0] < w[1]) && ranks.iter().all(|r| r.as_str().len() <= Rank::MAX_LEN)
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
    cx.ensure(
        order(cx, list.id).await == [c.id, b.id, a.id],
        "create: a new task is on top of its column",
    );
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
    spread(cx).await;
}

/// 1000 appends at the top (`create`) and at the bottom (`place`): short ranks, nobody else is written.
async fn appends(cx: &mut Cx<'_>) {
    let dir = cx.dir;
    let Some(list) = cx.list("rank top").await else {
        return;
    };
    let Some(first) = cx.task(list.id, "first").await else {
        return;
    };
    for i in 0..1000 {
        if dir
            .create(super::cx::draft(
                list.id,
                &format!("t{i}"),
                TaskPatch::default(),
            ))
            .await
            .is_err()
        {
            cx.ensure(false, "create: 1000 appends at the top");
            return;
        }
    }
    cx.ensure(
        ascending(cx, list.id).await,
        "rank: 1000 appends at the top stay ascending and short",
    );
    cx.ensure(
        cx.fresh(first.id)
            .await
            .is_some_and(|f| f.revision == first.revision),
        "rank: 1000 appends at the top write nobody else",
    );
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
        ascending(cx, list.id).await,
        "rank: 1000 appends at the bottom stay ascending and short",
    );
    cx.ensure(
        order(cx, list.id).await.last() == Some(&last),
        "rank: the last appended task is last",
    );
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
    let Ok(start) = dir.changes(None, 0).await else {
        return;
    };
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
    }
    cx.ensure(
        moved_without_changed,
        "place: the spread is in the same transaction, no Changed for the caller",
    );
    cx.ensure(
        ascending(cx, list.id).await,
        "rank: after the spread the order is ascending",
    );
    let (Some(l), Some(h)) = (cx.fresh(low.id).await, cx.fresh(high.id).await) else {
        return;
    };
    cx.ensure(
        l.revision > low.revision || h.revision > high.revision,
        "rank: the spread gives the tasks whose rank changed a new revision",
    );
    let moves = dir
        .changes(start.next.as_ref(), 100_000)
        .await
        .map(|p| p.items)
        .unwrap_or_default();
    let by_spread = moves
        .iter()
        .filter(|c| c.kind == HistoryKind::Move && (c.task == low.id || c.task == high.id))
        .count();
    cx.ensure(
        by_spread > 0,
        "changes: a Move for each task the spread rewrote",
    );
    let log = dir
        .history(low.id, None, 100)
        .await
        .map(|p| p.items)
        .unwrap_or_default();
    cx.ensure(
        !log.iter().any(|e| e.kind == HistoryKind::Move),
        "history: the spread writes no Move into the log of a task nobody moved",
    );
}
