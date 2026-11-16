//! The runners of the conformance rules.
//!
//! Pattern: Template method (a fixed list of rules, each one checked against the provider under test).

pub use super::cx::Report;
use super::cx::{Cx, Options};
use super::{
    ConformanceFixture, rules_assign, rules_checklist, rules_claim, rules_claim_order, rules_due,
    rules_feed, rules_groups, rules_labels, rules_leak, rules_lists, rules_off, rules_parts,
    rules_purge, rules_query, rules_rank, rules_rank_top, rules_readonly, rules_ready,
    rules_repeat, rules_repeat_dates, rules_repeat_more, rules_series, rules_tasks, rules_trash,
};
use crate::tasks_v1::{CAPABILITIES, Tasks};

async fn base(cx: &mut Cx<'_>) {
    for cap in CAPABILITIES {
        let on = cx.caps(cap);
        if cx.partial && !on {
            cx.skip(&format!("capabilities: {cap} is not offered (partial)"));
            continue;
        }
        cx.ensure(
            on || cap == "dependency.evaluate",
            &format!("capabilities: {cap} is offered"),
        );
    }
    rules_lists::rules(cx).await;
    rules_tasks::rules(cx).await;
    if cx.needs("assign", &["task.assign"]) {
        rules_assign::rules(cx).await;
    }
    rules_trash::rules(cx).await;
    rules_query::rules(cx).await;
    rules_due::rules(cx).await;
    if cx.needs("parts", &["task.edit"]) {
        rules_parts::rules(cx).await;
    }
    if cx.needs("series", &["task.progress"]) && cx.needs_series("series").await {
        rules_series::rules(cx).await;
    }
    if cx.needs("ready", &["relation.create"]) {
        rules_ready::rules(cx).await;
    }
    if cx.needs("labels", &["task.labels"]) {
        rules_labels::rules(cx).await;
    }
    if cx.needs("feed", &["task.changes"]) {
        rules_feed::rules(cx).await;
    }
    if !cx.skip_rank && cx.needs("rank", &["task.place"]) {
        Box::pin(rules_rank::rules(cx)).await;
    }
    if cx.needs("checklist", &["task.checklist"]) {
        Box::pin(rules_checklist::rules(cx)).await;
    }
    if cx.needs("repeat", &["task.progress"]) && cx.needs_series("repeat").await {
        rules_repeat::rules(cx).await;
    }
    if cx.needs("claim", &["task.claim"]) {
        Box::pin(rules_claim::rules(cx)).await;
    }
    rules_purge::rules(cx).await;
    rules_off::rules(cx).await;
}

/// The two parts of the rank rules (they write thousands of tasks, so a slow provider runs them as tests of their own).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RankPart {
    /// New tasks on top, the top edge and the spread on top: `create` only (no `place` needed).
    Top,
    /// Neighbours, the bottom edge and the spread by placing: needs `task.place`.
    Place,
}

/// Only the rank rules of one part (`skip_rank` of `options` is ignored).
pub async fn run_rank_conformance(
    fixture: &dyn ConformanceFixture,
    part: RankPart,
    options: Options,
) -> Report {
    let Some(mut cx) = Cx::new(fixture.tasks(), options) else {
        return Report {
            failures: vec!["setup: the zones UTC and Europe/Berlin".to_owned()],
            skipped: Vec::new(),
            checked: 0,
        };
    };
    match part {
        RankPart::Top => rules_rank_top::rules(&mut cx, Some(fixture)).await,
        RankPart::Place => {
            if cx.needs("rank", &["task.place"]) {
                Box::pin(rules_rank::rules(&mut cx)).await;
            }
        }
    }
    cx.report
}

/// Run every rule that needs only the trait against `dir` (data is seeded through `create_list` and `create`, so
/// the provider needs `list.create` and `task.create`). A provider without `dependency.evaluate` gets the rules for
/// that case instead. Returns the failed rules (empty = conformant).
pub async fn run_conformance(dir: &dyn Tasks) -> Vec<String> {
    run_conformance_with(dir, Options::default()).await
}

/// [`run_conformance`] with switches (`heavy` for the night run).
pub async fn run_conformance_with(dir: &dyn Tasks, options: Options) -> Vec<String> {
    let Some(mut cx) = Cx::new(dir, options) else {
        return vec!["setup: the zones UTC and Europe/Berlin".to_owned()];
    };
    base(&mut cx).await;
    if !cx.skip_rank {
        rules_rank_top::rules(&mut cx, None).await;
    }
    cx.report.failures
}

/// All rules: those of [`run_conformance`] plus the ones that need a state the fixture sets up (a clock past the
/// trash deadline, the "I" card and the account).
pub async fn run_fixture_conformance(fixture: &dyn ConformanceFixture) -> Report {
    run_fixture_conformance_with(fixture, Options::default()).await
}

/// [`run_fixture_conformance`] with switches (`heavy` for the night run).
pub async fn run_fixture_conformance_with(
    fixture: &dyn ConformanceFixture,
    options: Options,
) -> Report {
    let Some(mut cx) = Cx::new(fixture.tasks(), options) else {
        return Report {
            failures: vec!["setup: the zones UTC and Europe/Berlin".to_owned()],
            skipped: Vec::new(),
            checked: 0,
        };
    };
    base(&mut cx).await;
    if !cx.skip_rank {
        rules_rank_top::rules(&mut cx, Some(fixture)).await;
    }
    rules_trash::moves(fixture, &mut cx).await;
    rules_trash::deadline(fixture, &mut cx).await;
    if cx.needs_series("repeat kept").await {
        rules_repeat_more::kept(fixture, &mut cx).await;
    }
    rules_purge::states(fixture, &mut cx).await;
    rules_groups::rules(fixture, &mut cx).await;
    rules_assign::card_rules(fixture, &mut cx).await;
    rules_leak::rules(fixture, &mut cx).await;
    rules_leak::restore_order(fixture, &mut cx).await;
    rules_readonly::rules(fixture, &mut cx).await;
    rules_claim_order::rules(fixture, &mut cx).await;
    if cx.needs_series("repeat dates").await {
        rules_repeat_dates::rules(fixture, &mut cx).await;
    }
    cx.report
}
