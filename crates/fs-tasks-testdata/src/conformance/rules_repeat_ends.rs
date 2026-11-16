//! The end of a series from completion: `Count`, `Until`, `Cancelled` and `Failed`.
//!
//! Pattern: Template method (rules as plain checks collected in the report).

use super::cx::Cx;
use super::rules_repeat::{finish, naive_of, rule, series, timed, wall_due};
use crate::sample::wall as wall_of;
use crate::tasks_v1::{
    Due, Frequency, OverrideChange, Progress, Recurrence, RepeatFrom, RuleEnd, SeriesScope,
    TaskPatch,
};

/// `Count` counts the completions; the last one finishes the task; `Until`; `Cancelled`/`Failed`.
pub(super) async fn ends(cx: &mut Cx<'_>, list: crate::tasks_v1::ObjectId) {
    let d = timed(cx, "2026-11-10T09:00:00");
    let Some(t) = series(
        cx,
        list,
        "count",
        Some(d.clone()),
        None,
        rule(Frequency::Daily, 1, RuleEnd::Count(2)),
    )
    .await
    else {
        return;
    };
    let Ok(one) = finish(cx, &t, Progress::Completed, SeriesScope::Whole).await else {
        return;
    };
    let rest =
        matches!(one.recurrence.first(), Some(Recurrence::Rule(r)) if r.end == RuleEnd::Count(1));
    cx.ensure(
        rest && one.progress == Progress::NeedsAction,
        "repeat: Count(2) after the first completion is Count(1), the rest from the new reference point",
    );
    let Ok(last) = finish(cx, &one, Progress::Completed, SeriesScope::Whole).await else {
        cx.ensure(false, "repeat: the last completion answers");
        return;
    };
    cx.ensure(
        last.progress == Progress::Completed
            && last.overrides.len() == 1
            && last.recurrence == one.recurrence
            && wall_due(&last) == wall_due(&one),
        "repeat: the last completion finishes the task, adds no exception, keeps the rule and the due",
    );
    let reopened = finish(cx, &last, Progress::NeedsAction, SeriesScope::Whole).await;
    cx.ensure(
        reopened.is_ok_and(|r| r.progress == Progress::NeedsAction),
        "repeat: a finished series is reopened with progress NeedsAction and Whole",
    );
    let until = RuleEnd::Until(wall_of("2026-11-12T09:00:00"));
    if let Some(u) = series(
        cx,
        list,
        "until",
        Some(d.clone()),
        None,
        rule(Frequency::Daily, 1, until),
    )
    .await
        && let Ok(done) = finish(cx, &u, Progress::Completed, SeriesScope::Whole).await
    {
        cx.ensure(
            done.progress == Progress::Completed && done.overrides.is_empty(),
            "repeat: when the next reference point lies after Until, completing finishes the task",
        );
    }
    cancel(cx, list, d).await;
}

async fn cancel(cx: &mut Cx<'_>, list: crate::tasks_v1::ObjectId, d: Due) {
    let Some(t) = series(
        cx,
        list,
        "cancel",
        Some(d.clone()),
        None,
        rule(Frequency::Daily, 1, RuleEnd::Never),
    )
    .await
    else {
        return;
    };
    let Some(rid) = naive_of(&t) else { return };
    let Ok(next) = finish(
        cx,
        &t,
        Progress::Cancelled,
        SeriesScope::This { recurrence_id: rid },
    )
    .await
    else {
        cx.ensure(false, "repeat: Cancelled with This answers");
        return;
    };
    cx.ensure(
        next.progress == Progress::NeedsAction
            && matches!(
                next.overrides.first().map(|o| &o.change),
                Some(OverrideChange::Changed {
                    progress: Some(Progress::Cancelled),
                    ..
                })
            ),
        "repeat: Cancelled with This moves on and writes the exception with that state",
    );
    let whole = cx
        .task_with(
            list,
            "cancel whole",
            TaskPatch {
                due: Some(Some(d)),
                recurrence: Some(vec![Recurrence::Rule(rule(
                    Frequency::Daily,
                    1,
                    RuleEnd::Never,
                ))]),
                repeat_from: Some(RepeatFrom::Completion),
                ..TaskPatch::default()
            },
        )
        .await;
    if let Some(w) = whole
        && let Ok(done) = finish(cx, &w, Progress::Failed, SeriesScope::Whole).await
    {
        cx.ensure(
            done.progress == Progress::Failed
                && done.overrides.is_empty()
                && wall_due(&done) == wall_due(&w),
            "repeat: Failed with Whole ends the series at once, no exception, no moving on",
        );
    }
}
