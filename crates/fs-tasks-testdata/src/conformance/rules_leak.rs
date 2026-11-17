//! Rules for what a person who may not see a list must not learn from it: `lists`, `get`, `history`, `changes` and
//! `purge` all answer as if the task did not exist (reading a list covers all four, `ListAccess::Read`), and the order
//! of the checks of `restore` is `NotFound`, then `Forbidden`, then `Invalid`.
//!
//! Pattern: Template method (rules as plain checks collected in the report). The leak probe: the colleague makes a
//! list for a group the AI is not in and deletes a task in it; the AI asks about everything.

use super::ConformanceFixture;
use super::cx::{Cx, is_forbidden, is_invalid, is_not_found};
use crate::tasks_v1::{Place, SeriesScope, Trash};

pub(super) async fn rules(fx: &dyn ConformanceFixture, cx: &mut Cx<'_>) {
    let (Some(colleague), Some(group), Some(agent)) = (
        fx.colleague().await,
        fx.foreign_group().await,
        fx.agent().await,
    ) else {
        cx.skip("leak: the fixture has no colleague, foreign group and agent");
        return;
    };
    if !cx.caps("task.history") || !cx.caps("trash.purge") {
        cx.skip("leak: the provider has no task.history or trash.purge");
        return;
    }
    if !fx.act_as(Some(colleague.clone())).await {
        cx.skip("leak: the fixture cannot act as the colleague");
        return;
    }
    let dir = cx.dir;
    let made = dir.create_list("Zzconf leak", None, Some(&group)).await;
    cx.ensure(
        made.is_ok(),
        "setup: the colleague makes a list for their group",
    );
    let Ok(list) = made else {
        fx.act_as(None).await;
        return;
    };
    let (Some(gone), Some(living)) = (
        cx.task(list.id, "deleted in the group list").await,
        cx.task(list.id, "living in the group list").await,
    ) else {
        fx.act_as(None).await;
        return;
    };
    let deleted = dir.delete(gone.id, gone.revision, SeriesScope::Whole).await;
    let start = Place("0".to_owned());
    let seen = cx.changes_since(Some(&start)).await;
    cx.ensure(
        deleted.is_ok() && seen.iter().any(|c| c.task == gone.id),
        "leak setup: the group sees the deletion in its own feed",
    );
    if !fx.act_as(Some(agent.clone())).await {
        cx.skip("leak: the fixture cannot act as the agent");
        fx.act_as(None).await;
        return;
    }
    asking(cx, &list.id, &gone.id, &living.id).await;
    fx.act_as(Some(colleague.clone())).await;
    cx.ensure(
        dir.purge(gone.id).await.is_ok(),
        "purge: the group itself still may (the AI's try changed nothing)",
    );
    after_list_purge(fx, cx, &colleague, &agent).await;
    fx.act_as(None).await;
}

/// A list that is purged takes its feed with it, for everybody: the AI, who never saw it, learns nothing afterwards
/// (ids, the list, create/add/delete, times), and the owner sees nothing of it either (purge makes no entry, so
/// hiding costs nobody anything).
async fn after_list_purge(
    fx: &dyn ConformanceFixture,
    cx: &mut Cx<'_>,
    colleague: &crate::tasks_v1::PersonRef,
    agent: &crate::tasks_v1::PersonRef,
) {
    let dir = cx.dir;
    if !fx.act_as(Some(colleague.clone())).await {
        cx.ensure(false, "leak setup: the fixture acts as the colleague again");
        return;
    }
    let Ok(list) = dir.create_list("Zzconf leak purged", None, None).await else {
        cx.ensure(false, "leak setup: create_list for the purged list");
        return;
    };
    let Some(t) = cx.task(list.id, "in a purged list").await else {
        return;
    };
    let rev = cx.fresh_list(list.id).await.unwrap_or(list.revision);
    let deleted = dir.delete_list(list.id, rev).await;
    cx.ensure(
        deleted.is_ok(),
        "leak setup: the colleague deletes the list",
    );
    let Ok(dl) = deleted else { return };
    cx.ensure(
        dir.purge(dl.id).await.is_ok(),
        "leak setup: the colleague purges the list",
    );
    let start = Place("0".to_owned());
    let own = cx.changes_since(Some(&start)).await;
    cx.ensure(
        !own.iter().any(|c| c.task == t.id || c.list == list.id),
        "leak: changes shows nobody (not the owner) what happened in a list that was purged",
    );
    if !fx.act_as(Some(agent.clone())).await {
        cx.ensure(false, "leak setup: the fixture acts as the agent again");
        return;
    }
    let seen = cx.changes_since(Some(&start)).await;
    cx.ensure(
        !seen.iter().any(|c| c.task == t.id || c.list == list.id),
        "leak: changes shows the AI nothing of a list that was purged",
    );
}

/// What the AI, who does not see the list, gets from each way of asking.
async fn asking(
    cx: &mut Cx<'_>,
    list: &crate::tasks_v1::ObjectId,
    gone: &crate::tasks_v1::ObjectId,
    living: &crate::tasks_v1::ObjectId,
) {
    let dir = cx.dir;
    let lists = dir.lists(Trash::Include).await.unwrap_or_default();
    cx.ensure(
        !lists.iter().any(|l| l.id == *list),
        "leak: lists does not show a list the asker may not see",
    );
    cx.ensure(
        matches!(dir.get(*living).await, Ok(None)) && matches!(dir.get(*gone).await, Ok(None)),
        "leak: get of a task in an invisible list is None",
    );
    cx.fails(
        &dir.history(*gone, None, 10).await,
        "leak: history of a task in an invisible list is NotFound",
        is_not_found,
    );
    cx.fails(
        &dir.history(*living, None, 10).await,
        "leak: history of a living task in an invisible list is NotFound",
        is_not_found,
    );
    let start = Place("0".to_owned());
    let feed = cx.changes_since(Some(&start)).await;
    cx.ensure(
        !feed.iter().any(|c| c.task == *gone || c.list == *list),
        "leak: changes leaves out what happened in a list the asker may not see",
    );
    cx.fails(
        &dir.purge(*gone).await,
        "leak: purge of a task in an invisible list is NotFound",
        is_not_found,
    );
    cx.fails(
        &dir.restore(*gone, SeriesScope::Whole).await,
        "restore: a deleted task in an invisible list is NotFound, not Invalid",
        is_not_found,
    );
    cx.fails(
        &dir.restore(*living, SeriesScope::Whole).await,
        "restore: a task in an invisible list is NotFound, not Invalid",
        is_not_found,
    );
}

/// The order of the checks of `restore`: `NotFound`, then `Forbidden`, then `Invalid` (nothing is given away).
pub(super) async fn restore_order(fx: &dyn ConformanceFixture, cx: &mut Cx<'_>) {
    let Ok(me) = cx.dir.asker().await else { return };
    let Some(list) = cx.list("restore order").await else {
        return;
    };
    let Some(t) = cx.task(list.id, "living, read only").await else {
        return;
    };
    if !fx.read_only(&me, list.id).await {
        cx.skip("restore: Forbidden before Invalid (the fixture cannot make a list read only)");
        return;
    }
    let answer = cx.dir.restore(t.id, SeriesScope::Whole).await;
    cx.fails(
        &answer,
        "restore: without the right to write it is Forbidden, before Invalid",
        |e| is_forbidden(e) && !is_invalid(e),
    );
}
