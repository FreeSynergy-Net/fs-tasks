//! Rules for the hand order that need only `create`: a new task is on top of its column, short ranks at the top edge,
//! and the one-time spread when the rank at the top runs out (Opus review K1-1, A2: these rules do not hang on
//! `task.place`, a provider without it runs them too). The rules with `place` are in `rules_rank`.
//!
//! Pattern: Template method (rules as plain checks collected in the report).

use super::cx::{Cx, draft};
use super::rules_rank::{ascending, edge_len, order};
use crate::tasks_v1::{Rank, TaskPatch};

pub(super) async fn rules(cx: &mut Cx<'_>) {
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
    top_edge_and_spread(cx).await;
}

/// Always on top of the column: after `Cx::edge` creations the ranks are short (the edge bound, nobody else written);
/// going on until the rank runs out (about 25 creations per character of the longest rank) the provider spreads the
/// column once, in the same transaction, no `Changed` for the caller, the order stays exactly the reverse of the
/// creation, and a task whose rank changed has a new revision.
async fn top_edge_and_spread(cx: &mut Cx<'_>) {
    let n = cx.edge;
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
        if i + 1 == n {
            cx.ensure(
                ascending(cx, list.id, edge_len(n)).await,
                &format!("rank: {n} insertions at the top stay ascending and at most {n}/25 + 2 characters long"),
            );
            cx.ensure(
                cx.fresh(first.id)
                    .await
                    .is_some_and(|f| f.revision == first.revision),
                "rank: insertions at the top write nobody else",
            );
        }
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
        "rank: the spread on top gives the tasks whose rank changed a new revision",
    );
}
