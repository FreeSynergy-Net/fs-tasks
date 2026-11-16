//! Rules for people: `asker`, `assign`, `unassign`, `respond`, and the filter `Me` (account AND "I" card).
//!
//! Pattern: Template method (rules as plain checks collected in the report).

use super::ConformanceFixture;
use super::cx::{Cx, is_forbidden};
use crate::ids::person;
use crate::tasks_v1::{Answer, AssigneeFilter, AssigneeStatus, ObjectId, SeriesScope};

pub(super) async fn rules(cx: &mut Cx<'_>) {
    let dir = cx.dir;
    let Ok(me) = dir.asker().await else {
        cx.ensure(false, "asker: answers without an error");
        return;
    };
    let Some(list) = cx.list("people").await else {
        return;
    };
    let Some(t) = cx.task(list.id, "Assign").await else {
        return;
    };
    let not_mine = dir
        .respond(t.id, t.revision, Answer::Accept, None, SeriesScope::Whole)
        .await;
    cx.fails(
        &not_mine,
        "respond: not assigned is Forbidden",
        is_forbidden,
    );

    let anna = person(9001);
    let Ok(a) = dir
        .assign(t.id, t.revision, &anna, SeriesScope::Whole)
        .await
    else {
        cx.ensure(false, "assign: adds the person");
        return;
    };
    let row = a.assignees.iter().find(|x| x.who == anna);
    cx.ensure(
        row.is_some_and(|r| {
            r.status == AssigneeStatus::NeedsAction && r.assigned_by.as_ref() == Some(&me)
        }),
        "assign: NeedsAction, assigned_by is the asking person",
    );
    let Ok(again) = dir
        .assign(t.id, a.revision, &anna, SeriesScope::Whole)
        .await
    else {
        cx.ensure(false, "assign: twice answers");
        return;
    };
    cx.ensure(
        again.revision == a.revision && again.assignees.len() == 1,
        "assign: twice changes nothing, no new revision",
    );
    let Ok(none) = dir
        .unassign(t.id, again.revision, &person(9002), SeriesScope::Whole)
        .await
    else {
        cx.ensure(false, "unassign: a person who is not assigned answers");
        return;
    };
    cx.ensure(
        none.revision == again.revision,
        "unassign: not assigned changes nothing, no new revision",
    );
    let who = dir
        .list(
            &{
                let mut q = cx.query(&[list.id]);
                q.assignee = Some(AssigneeFilter::Person(anna.clone()));
                q
            },
            None,
            10,
        )
        .await;
    cx.ensure(
        who.is_ok_and(|p| p.items.len() == 1),
        "list: AssigneeFilter::Person",
    );
    let nobody = {
        let mut q = cx.query(&[list.id]);
        q.assignee = Some(AssigneeFilter::Nobody);
        q
    };
    cx.ensure(
        cx.all(&nobody).await.is_empty(),
        "list: AssigneeFilter::Nobody leaves the assigned task out",
    );
    let Ok(gone) = dir
        .unassign(t.id, none.revision, &anna, SeriesScope::Whole)
        .await
    else {
        cx.ensure(false, "unassign: removes the person");
        return;
    };
    cx.ensure(
        gone.assignees.is_empty() && gone.revision > none.revision,
        "unassign: removed, revision up",
    );
    me_rules(cx, list.id, &me).await;
}

/// "Mir zuweisen" and the answer of the asking person.
async fn me_rules(cx: &mut Cx<'_>, list: ObjectId, me: &crate::tasks_v1::PersonRef) {
    let dir = cx.dir;
    let Some(t) = cx.task(list, "Mine").await else {
        return;
    };
    let Ok(t) = dir.assign(t.id, t.revision, me, SeriesScope::Whole).await else {
        cx.ensure(false, "assign: the asking person");
        return;
    };
    let mut q = cx.query(&[list]);
    q.assignee = Some(AssigneeFilter::Me);
    cx.ensure(
        cx.ids(&q).await == [t.id],
        "list: AssigneeFilter::Me finds what is assigned to the asker",
    );
    let accepted = dir
        .respond(
            t.id,
            t.revision,
            Answer::Accept,
            Some("gern"),
            SeriesScope::Whole,
        )
        .await;
    let row = accepted
        .as_ref()
        .ok()
        .and_then(|x| x.assignees.iter().find(|a| a.who == *me));
    cx.ensure(
        row.is_some_and(|r| {
            r.status == AssigneeStatus::Accepted && r.comment.as_deref() == Some("gern")
        }),
        "respond: Accept with a comment writes the asker's row",
    );
    let Ok(t) = accepted else { return };
    let declined = dir
        .respond(t.id, t.revision, Answer::Decline, None, SeriesScope::Whole)
        .await;
    cx.ensure(
        declined.is_ok_and(|x| {
            x.assignees
                .iter()
                .any(|a| a.who == *me && a.status == AssigneeStatus::Declined)
        }),
        "respond: Decline",
    );
}

/// `Me` meets the account AND the "I" card of the same person (PLAN section 11).
pub(super) async fn card_rules(fx: &dyn ConformanceFixture, cx: &mut Cx<'_>) {
    let dir = cx.dir;
    let Some(account) = fx.account().await else {
        cx.skip("asker: the account when there is no I card (fixture has no account)");
        return;
    };
    let card = person(9100);
    if !fx.set_me_card(None).await || !fx.set_me_card(Some(card.clone())).await {
        cx.skip("asker: the I card (fixture cannot set it)");
        return;
    }
    cx.ensure(
        dir.asker().await.is_ok_and(|a| a == card),
        "asker: the I card when there is one",
    );
    let Some(list) = cx.list("card").await else {
        return;
    };
    let (Some(on_account), Some(on_card)) = (
        cx.task(list.id, "On account").await,
        cx.task(list.id, "On card").await,
    ) else {
        return;
    };
    let first = dir
        .assign(
            on_account.id,
            on_account.revision,
            &account,
            SeriesScope::Whole,
        )
        .await;
    let second = dir
        .assign(on_card.id, on_card.revision, &card, SeriesScope::Whole)
        .await;
    cx.ensure(
        first.is_ok() && second.is_ok(),
        "setup: assign to account and card",
    );
    let mut q = cx.query(&[list.id]);
    q.assignee = Some(AssigneeFilter::Me);
    let mut got = cx.ids(&q).await;
    let mut want = vec![on_account.id, on_card.id];
    got.sort();
    want.sort();
    cx.ensure(
        got == want,
        "list: Me meets the account AND the I card of the same person",
    );
    cx.ensure(fx.set_me_card(None).await, "setup: remove the I card");
    cx.ensure(
        dir.asker().await.is_ok_and(|a| a == account),
        "asker: without an I card the account, never an error",
    );
}
