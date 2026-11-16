//! Rules for "repeat from completion" (`RepeatFrom::Completion`): counting from the day of the completion, the end of the
//! series, cancelling, and the form of the rule.
//!
//! Pattern: Template method (rules as plain checks collected in the report). The day of the completion is read from
//! `progress_updated` of the answer, so the rules need no clock of their own.

use super::cx::Cx;
use crate::clock::{local, wall};
use crate::sample::wall as wall_of;
use crate::tasks_v1::{
    Children, Due, Frequency, OverrideChange, Progress, Recurrence, RecurrenceRule, RepeatFrom,
    RuleEnd, SeriesScope, Task, TaskPatch,
};
use chrono::{DateTime, Duration, Months, NaiveDate, NaiveDateTime};

pub(super) fn rule(frequency: Frequency, interval: u32, end: RuleEnd) -> RecurrenceRule {
    RecurrenceRule {
        end,
        ..RecurrenceRule::every(frequency, interval)
    }
}

pub(super) fn timed(cx: &Cx<'_>, text: &str) -> Due {
    Due::Timed {
        at: wall_of(text),
        zone: cx.berlin.clone(),
        zone_assumed: false,
    }
}

/// A series from completion with `due` (and `start`) and the rule.
pub(super) async fn series(
    cx: &mut Cx<'_>,
    list: crate::tasks_v1::ObjectId,
    title: &str,
    due: Option<Due>,
    start: Option<Due>,
    rule: RecurrenceRule,
) -> Option<Task> {
    let rest = TaskPatch {
        due: due.map(Some),
        start: start.map(Some),
        recurrence: Some(vec![Recurrence::Rule(rule)]),
        repeat_from: Some(RepeatFrom::Completion),
        ..TaskPatch::default()
    };
    cx.task_with(list, title, rest).await
}

/// Mark done (`Whole`) or with an explicit state and scope.
pub(super) async fn finish(
    cx: &Cx<'_>,
    t: &Task,
    to: Progress,
    scope: SeriesScope,
) -> crate::tasks_v1::TaskResult<Task> {
    cx.dir
        .progress(t.id, t.revision, to, Children::Leave, scope)
        .await
}

/// The local day of the completion (in Berlin, the zone of the series), from `progress_updated`.
pub(super) fn done_day(t: &Task) -> Option<NaiveDate> {
    let ms = t.progress_updated?.millis();
    Some(
        DateTime::from_timestamp_millis(ms)?
            .with_timezone(&chrono_tz::Europe::Berlin)
            .date_naive(),
    )
}

pub(super) fn wall_due(t: &Task) -> Option<NaiveDateTime> {
    t.due.as_ref().and_then(wall)
}

fn at(day: NaiveDate, hh_mm: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(&format!("{day}T{hh_mm}:00"), "%Y-%m-%dT%H:%M:%S").ok()
}

pub(super) async fn rules(cx: &mut Cx<'_>) {
    if !cx.caps("task.progress") {
        return;
    }
    let Some(list) = cx.list("repeat").await else {
        return;
    };
    counting(cx, list.id).await;
    super::rules_repeat_ends::ends(cx, list.id).await;
    super::rules_repeat_more::rules(cx, list.id).await;
}

/// The next due date counts from the day of the completion plus the interval; the time stays; late and early do not matter.
async fn counting(cx: &mut Cx<'_>, list: crate::tasks_v1::ObjectId) {
    let cases = [
        ("daily 3", Frequency::Daily, 3, "2026-11-10T09:00:00"),
        ("overdue", Frequency::Daily, 3, "2020-01-01T09:00:00"),
        ("early", Frequency::Daily, 3, "2031-01-01T09:00:00"),
        ("weekly 2", Frequency::Weekly, 2, "2026-11-10T09:00:00"),
        ("monthly 1", Frequency::Monthly, 1, "2026-01-31T09:00:00"),
        ("yearly 1", Frequency::Yearly, 1, "2024-02-29T09:00:00"),
    ];
    for (name, freq, n, due) in cases {
        let d = timed(cx, due);
        let Some(t) = series(cx, list, name, Some(d), None, rule(freq, n, RuleEnd::Never)).await
        else {
            continue;
        };
        let before = cx
            .dir
            .history(t.id, None, 100)
            .await
            .map_or(0, |p| p.items.len());
        let Ok(next) = finish(cx, &t, Progress::Completed, SeriesScope::Whole).await else {
            cx.ensure(false, &format!("repeat {name}: completing answers"));
            continue;
        };
        let Some(done) = done_day(&next) else {
            continue;
        };
        let want = step(freq, n, done).and_then(|x| at(x, "09:00"));
        cx.ensure(
            wall_due(&next) == want,
            &format!("repeat {name}: the next due is the day of the completion plus the interval, time of day unchanged"),
        );
        cx.ensure(
            next.id == t.id
                && next.uid == t.uid
                && next.sequence == t.sequence + 1
                && next.progress == Progress::NeedsAction,
            &format!("repeat {name}: the SAME task goes on (id, uid, sequence + 1, open again)"),
        );
        let old = naive_of(&t);
        cx.ensure(
            next.overrides.len() == 1
                && old.is_some_and(|o| next.overrides[0].recurrence_id == o)
                && matches!(
                    &next.overrides[0].change,
                    OverrideChange::Changed { progress: Some(Progress::Completed), .. }
                ),
            &format!("repeat {name}: the finished occurrence stays as an exception Changed(Completed) at its old reference point"),
        );
        let log = cx
            .dir
            .history(t.id, None, 100)
            .await
            .map(|p| p.items)
            .unwrap_or_default();
        cx.ensure(
            log.len() == before + 1
                && log
                    .last()
                    .is_some_and(|e| e.kind == crate::tasks_v1::HistoryKind::Update),
            &format!("repeat {name}: one call writes one Update entry"),
        );
    }
    start_and_due(cx, list).await;
}

/// The rule's date arithmetic as plain `chrono` (months and years clamp to the end of the month).
fn step(freq: Frequency, n: u32, day: NaiveDate) -> Option<NaiveDate> {
    match freq {
        Frequency::Daily => day.checked_add_signed(Duration::days(i64::from(n))),
        Frequency::Weekly => day.checked_add_signed(Duration::weeks(i64::from(n))),
        Frequency::Monthly => day.checked_add_months(Months::new(n)),
        Frequency::Yearly => day.checked_add_months(Months::new(n * 12)),
        _ => None,
    }
}

pub(super) fn naive_of(t: &Task) -> Option<crate::tasks_v1::LocalDateTime> {
    t.start
        .as_ref()
        .or(t.due.as_ref())
        .and_then(wall)
        .and_then(local)
}

/// With `start` and `due` both move by the same distance.
async fn start_and_due(cx: &mut Cx<'_>, list: crate::tasks_v1::ObjectId) {
    let (s, d) = (
        timed(cx, "2026-11-09T09:00:00"),
        timed(cx, "2026-11-10T17:00:00"),
    );
    let Some(t) = series(
        cx,
        list,
        "both",
        Some(d),
        Some(s),
        rule(Frequency::Daily, 2, RuleEnd::Never),
    )
    .await
    else {
        return;
    };
    let Ok(next) = finish(cx, &t, Progress::Completed, SeriesScope::Whole).await else {
        return;
    };
    let gap = |t: &Task| {
        let st = t.start.as_ref().and_then(wall)?;
        Some(t.due.as_ref().and_then(wall)? - st)
    };
    cx.ensure(
        gap(&t).is_some() && gap(&t) == gap(&next),
        "repeat: with start and due both move, the distance between them stays",
    );
    let day = done_day(&next).and_then(|x| x.checked_add_signed(Duration::days(2)));
    cx.ensure(
        next.start.as_ref().and_then(wall) == day.and_then(|x| at(x, "09:00")),
        "repeat: the reference point is start, counted from the completion",
    );
}
