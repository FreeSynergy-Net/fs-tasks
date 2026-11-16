//! The runners of the conformance rules.
//!
//! Pattern: Template method (a fixed list of rules, each one checked against the provider under test).

pub use super::cx::Report;
use super::cx::{Cx, Options, is_unsupported};
use super::{
    ConformanceFixture, rules_assign, rules_checklist, rules_claim, rules_claim_order, rules_feed,
    rules_groups, rules_labels, rules_leak, rules_lists, rules_off, rules_purge, rules_query,
    rules_rank, rules_rank_top, rules_readonly, rules_ready, rules_repeat, rules_repeat_dates,
    rules_repeat_more, rules_series, rules_tasks, rules_trash,
};
use crate::tasks_v1::{
    Answer, CAPABILITIES, Children, DependencyKind, Progress, SeriesScope, TaskDraft,
    TaskListPatch, TaskPatch, Tasks, Trash,
};

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
    if !cx.skip_rank {
        rules_rank_top::rules(cx).await;
        if cx.needs("rank", &["task.place"]) {
            rules_rank::rules(cx).await;
        }
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
pub async fn run_rank_conformance(dir: &dyn Tasks, part: RankPart, options: Options) -> Report {
    let Some(mut cx) = Cx::new(dir, options) else {
        return Report {
            failures: vec!["setup: the zones UTC and Europe/Berlin".to_owned()],
            skipped: Vec::new(),
            checked: 0,
        };
    };
    match part {
        RankPart::Top => rules_rank_top::rules(&mut cx).await,
        RankPart::Place => {
            if cx.needs("rank", &["task.place"]) {
                rules_rank::rules(&mut cx).await;
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

/// Rules for a provider WITHOUT the write capabilities: every method with a standard answer says `Unsupported`
/// (Null Object), and `capabilities` says the same.
pub async fn run_unsupported(dir: &dyn Tasks) -> Vec<String> {
    let Some(mut cx) = Cx::new(dir, Options::default()) else {
        return vec!["setup: the zones UTC and Europe/Berlin".to_owned()];
    };
    let id = crate::ids::object_id(1);
    let rev = crate::tasks_v1::Revision(1);
    let me = crate::ids::person(1);
    let draft = TaskDraft {
        list: id,
        uid: None,
        title: "x".to_owned(),
        progress: Progress::NeedsAction,
        assignees: Vec::new(),
        checklist: Vec::new(),
        rest: TaskPatch::default(),
    };
    let w = SeriesScope::Whole;
    let off: [(&str, bool); 8] = [
        (
            "list.create",
            is_unsupported_of(&dir.create_list("x", None, None).await),
        ),
        (
            "list.edit",
            is_unsupported_of(&dir.update_list(id, rev, TaskListPatch::default()).await),
        ),
        (
            "list.delete",
            is_unsupported_of(&dir.delete_list(id, rev).await),
        ),
        ("task.create", is_unsupported_of(&dir.create(draft).await)),
        (
            "task.edit",
            is_unsupported_of(&dir.update(id, rev, TaskPatch::default(), w).await),
        ),
        (
            "task.assign",
            is_unsupported_of(&dir.assign(id, rev, &me, w).await),
        ),
        (
            "task.respond",
            is_unsupported_of(&dir.respond(id, rev, Answer::Accept, None, w).await),
        ),
        (
            "task.progress",
            is_unsupported_of(
                &dir.progress(id, rev, Progress::Completed, Children::Leave, w)
                    .await,
            ),
        ),
    ];
    for (cap, unsupported) in off {
        cx.ensure(
            unsupported,
            &format!("{cap}: the default answer is Unsupported"),
        );
        let off = !cx.caps(cap);
        cx.ensure(off, &format!("capabilities: {cap} is off"));
    }
    let more = [
        (
            "task.place",
            is_unsupported_of(&dir.place(id, rev, None).await),
        ),
        (
            "relation.create",
            is_unsupported_of(
                &dir.relate(
                    id,
                    rev,
                    crate::tasks_v1::Dependency {
                        other_uid: "u".into(),
                        kind: DependencyKind::DependsOn,
                        gap: None,
                    },
                )
                .await,
            ),
        ),
        (
            "task.claim",
            is_unsupported_of(
                &dir.claim(id, rev, crate::tasks_v1::Utc::from_millis(1))
                    .await,
            ),
        ),
        (
            "task.delete",
            is_unsupported_of(&dir.delete(id, rev, w).await),
        ),
        ("task.restore", is_unsupported_of(&dir.restore(id, w).await)),
        (
            "task.history",
            is_unsupported_of(&dir.history(id, None, 1).await),
        ),
        (
            "task.labels",
            is_unsupported_of(&dir.labels(&[], None, None, 1).await),
        ),
        (
            "dependency.evaluate",
            is_unsupported_of(&dir.readiness(&[id]).await),
        ),
        (
            "task.checklist",
            is_unsupported_of(
                &dir.checklist(
                    id,
                    rev,
                    crate::tasks_v1::ChecklistChange::Remove { key: "k".into() },
                )
                .await,
            ),
        ),
        (
            "task.claim",
            is_unsupported_of(
                &dir.claim_next(&cx.query(&[]), crate::tasks_v1::Utc::from_millis(1))
                    .await,
            ),
        ),
        ("trash.purge", is_unsupported_of(&dir.purge(id).await)),
    ];
    for (cap, unsupported) in more {
        cx.ensure(
            unsupported,
            &format!("{cap}: the default answer is Unsupported"),
        );
    }
    let q = cx.query(&[]);
    cx.ensure(
        dir.lists(Trash::Exclude).await.is_ok() && dir.list(&q, None, 1).await.is_ok(),
        "required methods answer",
    );
    cx.ensure(dir.asker().await.is_ok(), "asker: answers without an error");
    cx.report.failures
}

fn is_unsupported_of<T>(r: &crate::tasks_v1::TaskResult<T>) -> bool {
    r.as_ref().err().is_some_and(is_unsupported)
}
