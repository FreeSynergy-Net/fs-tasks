//! A rule that cannot fail proves nothing: every new rule of the delta (checklist, claim, repeat from completion, purge,
//! subtask depth, owner) has a mutation of the fake that breaks exactly that behaviour, and the rule must say so.

mod common;

use common::Fx;
use fs_tasks_testdata::conformance::{Options, run_fixture_conformance_with};
use fs_tasks_testdata::{FakeTasks, Mutation};

/// Run the rules against a fake with `mutation`; the named rule must be among the failures (a part of its text).
async fn caught(mutation: Mutation, rule: &str) {
    let fx = Fx(FakeTasks::new().with_mutation(mutation));
    let options = Options {
        heavy: false,
        skip_rank: true,
    };
    let report = Box::pin(run_fixture_conformance_with(&fx, options)).await;
    assert!(
        report.failures.iter().any(|f| f.contains(rule)),
        "{mutation:?} was not caught by `{rule}`; failed rules: {:#?}",
        report.failures
    );
}

#[tokio::test]
async fn a_checklist_without_a_limit_is_caught() {
    caught(
        Mutation::ChecklistNoLimit,
        "item number CHECKLIST_MAX + 1 is Invalid",
    )
    .await;
}

#[tokio::test]
async fn ticks_that_stay_after_moving_on_are_caught() {
    caught(
        Mutation::TicksStay,
        "moving on takes the ticks off the checklist",
    )
    .await;
}

#[tokio::test]
async fn a_claim_rule_open_to_every_group_is_caught() {
    caught(
        Mutation::GroupOpen,
        "a group the asker is not in is Forbidden",
    )
    .await;
}

#[tokio::test]
async fn an_expired_claim_that_still_holds_is_caught() {
    caught(Mutation::ExpiredHolds, "an expired claim holds nobody").await;
}

#[tokio::test]
async fn an_ai_that_may_own_a_list_is_caught() {
    caught(Mutation::AiOwns, "an AI as owner is Forbidden").await;
}

#[tokio::test]
async fn counting_from_the_old_due_date_is_caught() {
    caught(
        Mutation::RepeatFromDue,
        "the next due is the day of the completion",
    )
    .await;
}

#[tokio::test]
async fn a_last_completion_that_does_not_finish_is_caught() {
    caught(
        Mutation::NoLastStop,
        "the last completion finishes the task",
    )
    .await;
}

#[tokio::test]
async fn a_purge_that_ignores_a_keep_is_caught() {
    caught(Mutation::PurgeHeld, "a kept task is Forbidden").await;
}

#[tokio::test]
async fn a_list_purge_that_leaves_tasks_behind_is_caught() {
    caught(Mutation::OrphansStay, "its tasks go with it (no view").await;
}

#[tokio::test]
async fn a_subtask_tree_without_a_depth_limit_is_caught() {
    caught(Mutation::NoDepthLimit, "SUBTASK_DEPTH_MAX + 1 is Invalid").await;
}
