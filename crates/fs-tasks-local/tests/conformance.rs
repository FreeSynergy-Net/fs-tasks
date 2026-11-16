//! Conformance Fake = Local for the rules run 1 covers: the same rules, written once in `fs-tasks-testdata`, run against
//! `LocalTasks`. The rules of the capabilities this run does not offer are skipped and named (never counted as passed).

#[allow(dead_code)]
mod common;

use common::LocalFx;
use fs_tasks_testdata::conformance::{Options, run_fixture_conformance_with};

fn partial() -> Options {
    Options {
        heavy: false,
        skip_rank: false,
        partial: true,
    }
}

#[tokio::test]
async fn local_tasks_pass_the_rules_run_1_covers() {
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
        skip_rank: true,
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
