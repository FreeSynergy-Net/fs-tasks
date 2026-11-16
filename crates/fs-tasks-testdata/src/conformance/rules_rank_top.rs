//! Rules for the hand order that need only `create`: a new task is on top of its column, short ranks at the top edge,
//! and the one-time spread when the rank at the top runs out (Opus review K1-1, A2: these rules do not hang on
//! `task.place`, a provider without it runs them too). The rules with `place` are in `rules_rank`.
//!
//! Pattern: Template method (rules as plain checks collected in the report).

use super::ConformanceFixture;
use super::cx::{Cx, draft};
use super::rules_rank::{ascending, edge_len, order};
use crate::tasks_v1::{Rank, TaskPatch};

pub(super) async fn rules(cx: &mut Cx<'_>, fx: Option<&dyn ConformanceFixture>) {
    let Some(list) = cx.list("rank new on top").await else {
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
    top_edge(cx).await;
    if let Some(fx) = fx {
        Box::pin(seeded_spread(fx, cx)).await;
    } else {
        cx.skip("rank: the spread on top by a seeded rank (no fixture)");
    }
    if cx.heavy {
        Box::pin(spread_by_creating(cx)).await;
    }
}

/// `Cx::edge` creations on top of each other: short ranks (the edge bound), nobody else is written.
async fn top_edge(cx: &mut Cx<'_>) {
    let n = cx.edge;
    let dir = cx.dir;
    let Some(list) = cx.list("rank edge top").await else {
        return;
    };
    let Some(first) = cx.task(list.id, "first").await else {
        return;
    };
    for i in 0..n {
        if dir
            .create(draft(list.id, &format!("t{i}"), TaskPatch::default()))
            .await
            .is_err()
        {
            cx.ensure(false, &format!("create: {n} insertions at the top"));
            return;
        }
    }
    cx.ensure(
        ascending(cx, list.id, edge_len(n)).await,
        &format!(
            "rank: {n} insertions at the top stay ascending and at most {n}/25 + 2 characters long"
        ),
    );
    cx.ensure(
        cx.fresh(first.id)
            .await
            .is_some_and(|f| f.revision == first.revision),
        "rank: insertions at the top write nobody else",
    );
}

/// The ranks a column gets when one task after the other is put on top: `(the last that still fits, the first that
/// does not)`; the next creation on top of the second one has to spread.
fn edge_ranks() -> Option<(Rank, Rank)> {
    let mut at = Rank::between(None, None).ok()?;
    loop {
        match Rank::between(None, Some(&at)) {
            Ok(next) => {
                if Rank::between(None, Some(&next)).is_err() {
                    return Some((at, next));
                }
                at = next;
            }
            Err(_) => return None,
        }
    }
}

/// The spread on top without thousands of creations: the top task gets a rank that does not fit any more (seeded by
/// the fixture); one creation on top then spreads the column once, in the same transaction (a new revision for the
/// task whose rank changed, the new task is first). The control: a rank that still fits spreads nothing.
async fn seeded_spread(fx: &dyn ConformanceFixture, cx: &mut Cx<'_>) {
    let Some((fits, full)) = edge_ranks() else {
        cx.ensure(false, "rank: the ranks of the top edge end at Exhausted");
        return;
    };
    for (rank, spreads, name) in [
        (full, true, "rank seeded full"),
        (fits, false, "rank seeded fits"),
    ] {
        let Some(list) = cx.list(name).await else {
            return;
        };
        let (Some(a), Some(b)) = (cx.task(list.id, "a").await, cx.task(list.id, "b").await) else {
            return;
        };
        if !fx.seed_rank(a.id, &rank).await {
            cx.skip("rank: the spread on top by a seeded rank (the fixture cannot seed a rank)");
            return;
        }
        let made = cx.task(list.id, "new on top").await;
        let Some(new) = made else {
            cx.ensure(
                false,
                "create: on top of a rank that does not fit spreads, no error",
            );
            return;
        };
        let order = cx.ids(&cx.query(&[list.id])).await;
        cx.ensure(
            order == [new.id, b.id, a.id] || order == [new.id, a.id, b.id],
            "rank: the new task is first after the spread on top",
        );
        let moved = cx
            .fresh(a.id)
            .await
            .is_some_and(|f| f.revision > a.revision);
        cx.ensure(
            moved == spreads,
            if spreads {
                "rank: the spread on top gives the tasks whose rank changed a new revision"
            } else {
                "rank: a rank that still fits spreads nothing"
            },
        );
    }
}

/// The night run: creations on top until the rank runs out for real (about 25 per character, 6362 and more).
async fn spread_by_creating(cx: &mut Cx<'_>) {
    let dir = cx.dir;
    let Some(list) = cx.list("rank on top").await else {
        return;
    };
    let Some(first) = cx.task(list.id, "first").await else {
        return;
    };
    let mut made = vec![first.id];
    for i in 0..Rank::MAX_LEN * 25 + 200 {
        let Ok(t) = dir
            .create(draft(list.id, &format!("s{i}"), TaskPatch::default()))
            .await
        else {
            cx.ensure(
                false,
                "create: the spread on top is in the same transaction",
            );
            return;
        };
        made.push(t.id);
    }
    made.reverse();
    cx.ensure(
        order(cx, list.id).await == made,
        "rank: after the spread on top the order is exactly the reverse of the creation",
    );
    cx.ensure(
        ascending(cx, list.id, Rank::MAX_LEN).await,
        "rank: after the spread on top the ranks are ascending and within the limit",
    );
    cx.ensure(
        cx.fresh(first.id)
            .await
            .is_some_and(|f| f.revision > first.revision),
        "rank: the spread on top gives the tasks whose rank changed a new revision (created)",
    );
}
