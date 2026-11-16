//! Conformance Fake = Local for the rules run 2a covers: the same rules, written once in `fs-tasks-testdata`, run against
//! `LocalTasks`. The rules of the capabilities this run does not offer are skipped and named (never counted as passed).
//!
//! Pattern: Conformance suite (the rules are written once in `fs-tasks-testdata`; this file only supplies the provider and the fixture).

#[allow(dead_code)]
mod common;

use common::LocalFx;
use fs_tasks_testdata::conformance::{
    Options, RankPart, run_fixture_conformance_with, run_rank_conformance,
};

/// The quick run: every group but the rank rules (they write thousands of tasks and run as tests of their own below).
fn partial() -> Options {
    Options {
        heavy: false,
        skip_rank: true,
        partial: true,
    }
}

#[tokio::test]
async fn local_tasks_pass_the_rules_run_2a_covers() {
    let fx = LocalFx::memory().await;
    let report = Box::pin(run_fixture_conformance_with(&fx, partial())).await;
    assert!(
        report.failures.is_empty(),
        "failed rules: {:#?}",
        report.failures
    );
    println!(
        "checked {}, skipped {}: {:#?}",
        report.checked,
        report.skipped.len(),
        report.skipped
    );
    assert!(
        report.checked > 150,
        "too few rules ran: {}",
        report.checked
    );
}

/// The staged mode is an honest switch, not a way out: the same provider run strict (the default) fails for every
/// capability it lacks, and run partial it names the skipped groups instead.
#[tokio::test]
async fn strict_mode_fails_what_partial_mode_skips() {
    let fx = LocalFx::memory().await;
    let strict = Options {
        partial: false,
        ..partial()
    };
    let report = Box::pin(run_fixture_conformance_with(&fx, strict)).await;
    assert!(
        report
            .failures
            .iter()
            .any(|f| f == "capabilities: task.assign is offered"),
        "a missing capability must fail in strict mode: {:#?}",
        report.failures
    );
    let fx = LocalFx::memory().await;
    let report = Box::pin(run_fixture_conformance_with(&fx, partial())).await;
    assert!(
        report
            .skipped
            .iter()
            .any(|s| s.starts_with("assign: the provider lacks task.assign")),
        "partial mode names the skipped group: {:#?}",
        report.skipped
    );
}

/// The rank rules that need only `create` (new on top, the top edge, the spread on top), against `LocalTasks`.
#[tokio::test]
async fn the_rank_rules_on_top_pass() {
    let fx = LocalFx::memory().await;
    let report = run_rank_conformance(&fx, RankPart::Top, partial()).await;
    assert!(report.failures.is_empty(), "failed: {:#?}", report.failures);
    println!("rank top: checked {}", report.checked);
    assert!(report.checked >= 7, "too few rules ran: {}", report.checked);
}

/// The rank rules with `place` (neighbours, the bottom, the spread by placing), against `LocalTasks`.
#[tokio::test]
async fn the_rank_rules_with_place_pass() {
    let fx = LocalFx::memory().await;
    let report = run_rank_conformance(&fx, RankPart::Place, partial()).await;
    assert!(report.failures.is_empty(), "failed: {:#?}", report.failures);
    println!("rank place: checked {}", report.checked);
    assert!(
        report.checked >= 10,
        "too few rules ran: {}",
        report.checked
    );
}

/// The night run of the top rank rules: creations on top until the rank runs out for real (6362 and more) and edges of
/// 6000. Slow in a debug build, so it is marked and runs once in CI (`cargo test -- --ignored`).
#[tokio::test]
#[ignore = "slow: the long rank rules run once in CI with --ignored"]
async fn the_rank_rules_on_top_pass_the_long_run() {
    let fx = LocalFx::memory().await;
    let options = Options {
        heavy: true,
        ..partial()
    };
    let report = run_rank_conformance(&fx, RankPart::Top, options).await;
    assert!(report.failures.is_empty(), "failed: {:#?}", report.failures);
}
