//! Rules that need groups, a second human or an AI (the fixture offers them): the claim rule over groups, `MyGroups`,
//! a claim of someone else, the AI rules, "STOP KI" and the owner of a list.
//!
//! Pattern: Template method (rules as plain checks collected in the report). A fixture without the mutator keeps the
//! default and the rule is reported as skipped, never as passed.

use super::ConformanceFixture;
use super::cx::{Cx, is_forbidden, is_invalid};
use super::rules_claim::{assigned, far};
use crate::tasks_v1::{AssigneeFilter, ObjectId, PersonRef, TaskError};

pub(super) async fn rules(fx: &dyn ConformanceFixture, cx: &mut Cx<'_>) {
    if !cx.caps("task.claim") {
        return;
    }
    let Some(list) = cx.list("groups").await else {
        return;
    };
    let Ok(me) = cx.dir.asker().await else { return };
    match (fx.my_group().await, fx.foreign_group().await) {
        (Some(mine), Some(foreign)) => group_claims(cx, list.id, &mine, &foreign).await,
        _ => cx.skip("claim: groups (the fixture has no my_group and foreign_group)"),
    }
    match fx.colleague().await {
        Some(c) => other_human(fx, cx, list.id, &me, &c).await,
        None => cx.skip("claim: held by another human (the fixture has no colleague)"),
    }
    match fx.agent().await {
        Some(a) => ai(fx, cx, list.id, &me, &a).await,
        None => cx.skip("claim: the AI rules (the fixture has no agent)"),
    }
    if let Some(mine) = fx.my_group().await {
        owner(cx, &mine).await;
    }
}

/// A member of an assigned group may take it (the assignees stay as they are); a non-member is `Forbidden`.
async fn group_claims(cx: &mut Cx<'_>, list: ObjectId, mine: &PersonRef, foreign: &PersonRef) {
    let dir = cx.dir;
    let (Some(a), Some(b)) = (
        assigned(cx, list, "my group", std::slice::from_ref(mine)).await,
        assigned(cx, list, "foreign group", std::slice::from_ref(foreign)).await,
    ) else {
        return;
    };
    let Ok(me) = dir.asker().await else { return };
    let got = dir.claim(a.id, a.revision, far(&a)).await;
    cx.ensure(
        got.as_ref().is_ok_and(|t| {
            t.claim.as_ref().is_some_and(|c| c.by == me)
                && t.assignees.len() == 1
                && t.assignees[0].who == *mine
        }),
        "claim: a member of the assigned group may take it; the claim is a person, the assignees do not change",
    );
    let no = dir.claim(b.id, b.revision, far(&b)).await;
    cx.fails(
        &no,
        "claim: a task assigned to a group the asker is not in is Forbidden",
        is_forbidden,
    );
    let mut q = cx.query(&[list]);
    q.assignee = Some(AssigneeFilter::MyGroups);
    let ids = cx.ids(&q).await;
    cx.ensure(
        ids == [a.id],
        "list: MyGroups shows the tasks of the groups the asker is in, not the other ones",
    );
    let unknown = dir
        .claim(
            crate::ids::object_id(u64::MAX - 1),
            crate::tasks_v1::Revision(1),
            far(&a),
        )
        .await;
    cx.ensure(
        matches!(
            unknown,
            Err(TaskError::Common(crate::tasks_v1::Error::NotFound(_)))
        ),
        "claim: the order is NotFound before Forbidden (an unknown task is not told as forbidden)",
    );
}

/// Another human holds it: `Claimed`; after the time it is free; the order is `Forbidden` before `Claimed`.
async fn other_human(
    fx: &dyn ConformanceFixture,
    cx: &mut Cx<'_>,
    list: ObjectId,
    me: &PersonRef,
    colleague: &PersonRef,
) {
    let dir = cx.dir;
    let both = [me.clone(), colleague.clone()];
    let Some(t) = assigned(cx, list, "two humans", &both).await else {
        return;
    };
    if !fx.act_as(Some(colleague.clone())).await {
        cx.skip("claim: held by another human (the fixture cannot act as the colleague)");
        return;
    }
    let theirs = dir.claim(t.id, t.revision, far(&t)).await;
    fx.act_as(None).await;
    let Ok(theirs) = theirs else {
        cx.skip("claim: held by another human (the colleague could not claim)");
        return;
    };
    let mine = dir.claim(t.id, theirs.revision, far(&t)).await;
    cx.ensure(
        matches!(&mine, Err(TaskError::Claimed(c)) if c.by == *colleague),
        "claim: held by another person is Claimed with their claim",
    );
    let release = dir.release(t.id, theirs.revision).await;
    cx.ensure(
        matches!(release, Err(TaskError::Claimed(_))),
        "release: only the holder gives it back while it holds",
    );
    if fx.advance_clock(500 * 86_400_000).await {
        let after = dir
            .claim(
                t.id,
                theirs.revision,
                crate::tasks_v1::Utc::from_millis(theirs.updated.millis() + 900 * 86_400_000),
            )
            .await;
        cx.ensure(after.is_ok(), "claim: an expired claim holds nobody");
    } else {
        cx.skip("claim: an expired claim holds nobody (the fixture cannot move the clock)");
    }
}

/// An AI: no list ownership, no human groups, "STOP KI" ends its claims.
async fn ai(
    fx: &dyn ConformanceFixture,
    cx: &mut Cx<'_>,
    list: ObjectId,
    me: &PersonRef,
    agent: &PersonRef,
) {
    let dir = cx.dir;
    let theirs = dir.create_list("Zzconf ai owner", None, Some(agent)).await;
    cx.fails(
        &theirs,
        "create_list: an AI as owner is Forbidden",
        is_forbidden,
    );
    let Some(unassigned) = cx.task(list, "for the ai").await else {
        return;
    };
    let Some(for_agent) =
        assigned(cx, list, "assigned to the ai", std::slice::from_ref(agent)).await
    else {
        return;
    };
    let Some(human) = assigned(cx, list, "human only", std::slice::from_ref(me)).await else {
        return;
    };
    if !fx.act_as(Some(agent.clone())).await {
        cx.skip("claim: the AI rules (the fixture cannot act as the agent)");
        return;
    }
    let own_list = dir.create_list("Zzconf ai own", None, None).await;
    cx.fails(
        &own_list,
        "create_list: an AI asking is Forbidden (no list of its own)",
        is_forbidden,
    );
    let no = dir.claim(human.id, human.revision, far(&human)).await;
    let free = dir
        .claim(unassigned.id, unassigned.revision, far(&unassigned))
        .await;
    let own = dir
        .claim(for_agent.id, for_agent.revision, far(&for_agent))
        .await;
    fx.act_as(None).await;
    cx.fails(
        &no,
        "claim: an AI does not take what a human is assigned to",
        is_forbidden,
    );
    cx.ensure(
        free.is_ok(),
        "claim: an AI takes what nobody is assigned to (with write access)",
    );
    cx.ensure(own.is_ok(), "claim: an AI takes what is assigned to it");
    if fx.stop_agent(agent).await {
        let after = cx.fresh(for_agent.id).await;
        cx.ensure(
            after.is_some_and(|a| a.claim.is_none()),
            "STOP KI: every claim of the AI ends at once",
        );
    } else {
        cx.skip("STOP KI (the fixture cannot withdraw the AI's rights)");
    }
}

/// A group the asker is in may own a list; another person may not.
async fn owner(cx: &mut Cx<'_>, group: &PersonRef) {
    let dir = cx.dir;
    let made = dir
        .create_list("Zzconf group list", None, Some(group))
        .await;
    cx.ensure(
        made.as_ref()
            .is_ok_and(|l| l.owner == *group && l.access >= crate::tasks_v1::ListAccess::Write),
        "create_list: a group the asker is in may own the list; the asker has at least Write",
    );
    let stranger = crate::ids::person(4242);
    let bad = dir
        .create_list("Zzconf bad owner", None, Some(&stranger))
        .await;
    cx.fails(
        &bad,
        "create_list: another person as owner is Invalid",
        is_invalid,
    );
}
