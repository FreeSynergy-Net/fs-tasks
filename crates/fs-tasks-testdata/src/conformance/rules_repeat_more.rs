//! More rules for "repeat from completion": the form of the rule, undo, the scope questions, what a move-on ends, skipping
//! an occurrence, the limit of kept exceptions, and the switch.
//!
//! Pattern: Template method (rules as plain checks collected in the report).

use super::ConformanceFixture;
use super::cx::{Cx, is_invalid};
use super::rules_repeat::{finish, naive_of, rule, series, timed};
use crate::tasks_v1::{
    Frequency, ObjectId, Progress, REPEAT_DONE_KEPT, Recurrence, RepeatFrom, RuleEnd, SeriesScope,
    Skip, TaskPatch,
};

pub(super) async fn rules(cx: &mut Cx<'_>, list: ObjectId) {
    forms(cx, list).await;
    super::rules_repeat_undo::undo_and_scope(cx, list).await;
    super::rules_repeat_undo::moving_on(cx, list).await;
    super::rules_repeat_undo::skipping(cx, list).await;
    switch(cx, list).await;
}

/// The rule of a series from completion keeps to its form; anything else is `Invalid` and nothing is written.
async fn forms(cx: &mut Cx<'_>, list: ObjectId) {
    let d = timed(cx, "2026-11-10T09:00:00");
    let make = |rules: Vec<Recurrence>, excluded: Vec<Recurrence>| TaskPatch {
        due: Some(Some(d.clone())),
        recurrence: Some(rules),
        excluded_recurrence: Some(excluded),
        repeat_from: Some(RepeatFrom::Completion),
        ..TaskPatch::default()
    };
    let day = || rule(Frequency::Daily, 1, RuleEnd::Never);
    let by_day = crate::tasks_v1::RecurrenceRule {
        by_month_day: vec![1],
        ..day()
    };
    let backward = crate::tasks_v1::RecurrenceRule {
        skip: Skip::Backward,
        ..day()
    };
    let cases = [
        (
            "Count(0)",
            make(
                vec![Recurrence::Rule(rule(
                    Frequency::Daily,
                    1,
                    RuleEnd::Count(0),
                ))],
                vec![],
            ),
        ),
        (
            "hourly",
            make(
                vec![Recurrence::Rule(rule(Frequency::Hourly, 1, RuleEnd::Never))],
                vec![],
            ),
        ),
        ("a by_ part", make(vec![Recurrence::Rule(by_day)], vec![])),
        (
            "a skip other than the default",
            make(vec![Recurrence::Rule(backward)], vec![]),
        ),
        (
            "two rules",
            make(
                vec![Recurrence::Rule(day()), Recurrence::Rule(day())],
                vec![],
            ),
        ),
        (
            "an exclusion",
            make(vec![Recurrence::Rule(day())], vec![Recurrence::Rule(day())]),
        ),
        (
            "an original text",
            make(
                vec![Recurrence::Original("FREQ=DAILY;BYHOUR=9".into())],
                vec![],
            ),
        ),
    ];
    for (name, rest) in cases {
        let r = cx.dir.create(super::cx::draft(list, "form", rest)).await;
        cx.fails(
            &r,
            &format!("repeat from completion with {name} is Invalid"),
            is_invalid,
        );
    }
}

/// At most `REPEAT_DONE_KEPT` exceptions stay on the task; the oldest go (they stay in the log only). Needs a clock that
/// moves (one completion a day), so it runs with a fixture.
pub(super) async fn kept(fx: &dyn ConformanceFixture, cx: &mut Cx<'_>) {
    if !cx.caps("task.progress") {
        return;
    }
    let Some(l) = cx.list("repeat kept").await else {
        return;
    };
    let d = timed(cx, "2026-11-10T09:00:00");
    let Some(mut t) = series(
        cx,
        l.id,
        "kept",
        Some(d),
        None,
        rule(Frequency::Daily, 1, RuleEnd::Never),
    )
    .await
    else {
        return;
    };
    for _ in 0..REPEAT_DONE_KEPT + 5 {
        if !fx.advance_clock(86_400_000).await {
            cx.skip("repeat: the limit of kept exceptions (the fixture cannot move the clock)");
            return;
        }
        let Ok(n) = finish(cx, &t, Progress::Completed, SeriesScope::Whole).await else {
            cx.ensure(false, "repeat: many completions in a row");
            return;
        };
        t = n;
    }
    cx.ensure(
        t.overrides.len() == REPEAT_DONE_KEPT,
        "repeat: the task keeps at most REPEAT_DONE_KEPT exceptions, the oldest are dropped",
    );
}

/// The switch between plan and completion is for the whole series only.
async fn switch(cx: &mut Cx<'_>, list: ObjectId) {
    let d = timed(cx, "2026-11-10T09:00:00");
    let Some(t) = series(
        cx,
        list,
        "switch",
        Some(d),
        None,
        rule(Frequency::Daily, 1, RuleEnd::Never),
    )
    .await
    else {
        return;
    };
    let Some(rid) = naive_of(&t) else { return };
    let whole_only = cx
        .dir
        .update(
            t.id,
            t.revision,
            TaskPatch {
                repeat_from: Some(RepeatFrom::Schedule),
                ..TaskPatch::default()
            },
            SeriesScope::This { recurrence_id: rid },
        )
        .await;
    cx.fails(
        &whole_only,
        "update: repeat_from with This is Invalid",
        is_invalid,
    );
    let switched = cx
        .dir
        .update(
            t.id,
            t.revision,
            TaskPatch {
                repeat_from: Some(RepeatFrom::Schedule),
                ..TaskPatch::default()
            },
            SeriesScope::Whole,
        )
        .await;
    cx.ensure(
        switched.is_ok_and(|s| s.repeat_from == RepeatFrom::Schedule),
        "update: repeat_from with Whole switches between plan and completion",
    );
}
