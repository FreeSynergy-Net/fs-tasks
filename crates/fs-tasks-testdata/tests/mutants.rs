//! A rule that cannot fail proves nothing: every new rule of the delta (checklist, claim, repeat from completion, purge,
//! subtask depth, owner) has a mutation of the fake that breaks exactly that behaviour, and the rule must say so.

mod common;

use common::Fx;
use fs_tasks_testdata::conformance::{
    Options, RankPart, run_fixture_conformance_with, run_rank_conformance,
};
use fs_tasks_testdata::{FakeTasks, Mutation};

/// Run the rules against a fake with `mutation`; the named rule must be among the failures (a part of its text).
async fn caught(mutation: Mutation, rule: &str) {
    let fx = Fx(FakeTasks::new().with_mutation(mutation));
    let options = Options {
        heavy: false,
        skip_rank: true,
        partial: false,
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

#[tokio::test]
async fn a_leak_of_history_changes_and_purge_is_caught() {
    caught(
        Mutation::LeakTrash,
        "leak: history of a task in an invisible list is NotFound",
    )
    .await;
    caught(
        Mutation::LeakTrash,
        "leak: changes leaves out what happened",
    )
    .await;
    caught(
        Mutation::LeakTrash,
        "leak: purge of a task in an invisible list is NotFound",
    )
    .await;
}

#[tokio::test]
async fn a_feed_of_a_purged_list_that_shows_is_caught() {
    caught(
        Mutation::LeakTrash,
        "leak: changes shows the AI nothing of a list that was purged",
    )
    .await;
    caught(
        Mutation::LeakTrash,
        "leak: changes shows nobody (not the owner)",
    )
    .await;
}

#[tokio::test]
async fn a_restore_without_the_depth_check_is_caught() {
    caught(
        Mutation::RestoreNoDepth,
        "restore: a task whose tree would be deeper than SUBTASK_DEPTH_MAX is Invalid",
    )
    .await;
}

#[tokio::test]
async fn tasks_everybody_sees_are_caught() {
    caught(
        Mutation::SeesAll,
        "leak: get of a task in an invisible list is None",
    )
    .await;
}

#[tokio::test]
async fn a_purge_that_takes_the_kept_subtask_along_is_caught() {
    caught(
        Mutation::PurgeTakesKept,
        "a kept subtask that went with the parent is detached",
    )
    .await;
}

#[tokio::test]
async fn a_claim_that_says_claimed_before_forbidden_is_caught() {
    caught(Mutation::ClaimedFirst, "Forbidden comes before Claimed").await;
}

#[tokio::test]
async fn a_release_of_an_expired_claim_that_skips_the_rule_is_caught() {
    caught(Mutation::ReleaseOpen, "an outsider gets Forbidden").await;
}

#[tokio::test]
async fn an_undo_at_any_exception_is_caught() {
    caught(
        Mutation::UndoAny,
        "undo of an older exception than the newest is Invalid",
    )
    .await;
}

#[tokio::test]
async fn a_claim_next_that_ignores_readiness_is_caught() {
    caught(
        Mutation::NextIgnoresReady,
        "a task with dependencies that is not ready is skipped",
    )
    .await;
}

#[tokio::test]
async fn a_restore_that_says_invalid_before_forbidden_is_caught() {
    caught(
        Mutation::RestoreInvalidFirst,
        "without the right to write it is Forbidden, before Invalid",
    )
    .await;
}

#[tokio::test]
async fn a_fixed_deadline_instead_of_the_time_of_the_delete_is_caught() {
    caught(
        Mutation::FixedDeadline,
        "the deadline of a task is the time of the delete plus the trash period",
    )
    .await;
    caught(
        Mutation::FixedDeadline,
        "the deadline of a list is the time of the delete plus the trash period",
    )
    .await;
}

/// A delete whose state is not `Until` is a failure for a full provider, not a skip (Opus review K1-1, A5).
#[tokio::test]
async fn a_delete_without_a_deadline_is_caught_in_the_strict_mode() {
    caught(
        Mutation::NoDeadline,
        "is `Until` (the time of the delete plus the trash period)",
    )
    .await;
}

/// The rank rules that need only `create` (Opus review K1-1, A2) against the two mutants of the top.
async fn caught_on_top(mutation: Mutation, rule: &str) {
    let fake = FakeTasks::new().with_mutation(mutation);
    let options = Options {
        heavy: false,
        skip_rank: false,
        partial: false,
    };
    let report = run_rank_conformance(&Fx(fake), RankPart::Top, options).await;
    assert!(
        report.failures.iter().any(|f| f.contains(rule)),
        "{mutation:?} was not caught by `{rule}`; failed rules: {:#?}",
        report.failures
    );
}

#[tokio::test]
async fn a_top_that_does_not_spread_is_caught() {
    Box::pin(caught_on_top(
        Mutation::TopNoSpread,
        "on top of a rank that does not fit spreads",
    ))
    .await;
}

#[tokio::test]
async fn a_spread_that_leaves_the_revisions_is_caught() {
    Box::pin(caught_on_top(
        Mutation::SpreadSilent,
        "the spread on top gives the tasks whose rank changed a new revision",
    ))
    .await;
}
