//! Rules for series: the current occurrence, `This`, `ThisAndFollowing`, the trash of one occurrence, the split.
//!
//! Pattern: Template method (rules as plain checks collected in the report).

use super::cx::{Cx, is_changed, is_invalid, is_unsupported};
use crate::occurrence;
use crate::tasks_v1::{
    Children, Due, Frequency, ObjectId, Progress, Recurrence, RecurrenceRule, Relation, RuleEnd,
    SeriesScope, Task, TaskError, TaskPatch,
};
use fs_types::BasicValue;

fn daily(count: u32) -> Vec<Recurrence> {
    vec![Recurrence::Rule(RecurrenceRule {
        end: RuleEnd::Count(count),
        ..RecurrenceRule::every(Frequency::Daily, 1)
    })]
}

fn on(day: &str) -> Option<Due> {
    crate::tasks_v1::Date::parse(day).ok().map(Due::Date)
}

async fn series(cx: &mut Cx<'_>, list: ObjectId, title: &str, count: u32) -> Option<Task> {
    let rest = TaskPatch {
        due: Some(on("2026-11-16")),
        recurrence: Some(daily(count)),
        ..Default::default()
    };
    cx.task_with(list, title, rest).await
}

fn this(day: &str) -> Option<SeriesScope> {
    occurrence(&format!("{day}T00:00:00")).map(|recurrence_id| SeriesScope::This { recurrence_id })
}

fn following(day: &str) -> Option<SeriesScope> {
    occurrence(&format!("{day}T00:00:00"))
        .map(|recurrence_id| SeriesScope::ThisAndFollowing { recurrence_id })
}

pub(super) async fn rules(cx: &mut Cx<'_>) {
    let dir = cx.dir;
    let Some(list) = cx.list("series").await else {
        return;
    };
    let Some(t) = series(cx, list.id, "Daily", 3).await else {
        return;
    };
    let (Some(d16), Some(d17), Some(d18), Some(d19)) = (
        this("2026-11-16"),
        this("2026-11-17"),
        this("2026-11-18"),
        this("2026-11-19"),
    ) else {
        return;
    };
    let s = cx.summary(list.id, t.id).await;
    cx.ensure(
        s.is_some_and(|s| {
            s.recurring
                && s.occurrence == occurrence("2026-11-16T00:00:00")
                && s.due == on("2026-11-16")
        }),
        "list: a series shows its current occurrence once",
    );
    let unknown = dir
        .progress(t.id, t.revision, Progress::Completed, Children::Leave, d19)
        .await;
    cx.fails(
        &unknown,
        "progress: an occurrence the rule does not make is Invalid",
        is_invalid,
    );
    let Ok(done) = dir
        .progress(t.id, t.revision, Progress::Completed, Children::Leave, d16)
        .await
    else {
        cx.ensure(false, "progress: This completes one occurrence");
        return;
    };
    cx.ensure(
        done.progress == Progress::NeedsAction,
        "progress: This leaves the series open",
    );
    let s = cx.summary(list.id, t.id).await;
    cx.ensure(
        s.is_some_and(|s| {
            s.occurrence == occurrence("2026-11-17T00:00:00") && s.due == on("2026-11-17")
        }),
        "progress: the next occurrence becomes the current one",
    );
    delete_rules(cx, list.id, &done, d17, d18).await;
    split_rules(cx, list.id).await;
    outside_subset(cx, list.id).await;
}

/// One occurrence in the trash and back; the last occurrence finishes a `Count` series.
async fn delete_rules(
    cx: &mut Cx<'_>,
    list: ObjectId,
    t: &Task,
    d17: SeriesScope,
    d18: SeriesScope,
) {
    let dir = cx.dir;
    let Ok(gone) = dir.delete(t.id, t.revision, d17).await else {
        cx.ensure(false, "delete: This makes an exception");
        return;
    };
    cx.ensure(
        gone.scope == d17 && gone.restorable.is_some(),
        "delete: This answers with its scope and a deadline",
    );
    let s = cx.summary(list, t.id).await;
    cx.ensure(
        s.is_some_and(|s| s.occurrence == occurrence("2026-11-18T00:00:00")),
        "delete: This skips the occurrence",
    );
    let Ok(back) = dir.restore(t.id, d17).await else {
        cx.ensure(false, "restore: with the scope of the delete");
        return;
    };
    let s = cx.summary(list, t.id).await;
    cx.ensure(
        s.is_some_and(|s| s.occurrence == occurrence("2026-11-17T00:00:00")),
        "restore: This gives the occurrence back",
    );
    let Some(d17) = this("2026-11-17") else {
        return;
    };
    let Ok(a) = dir
        .progress(
            back.id,
            back.revision,
            Progress::Completed,
            Children::Leave,
            d17,
        )
        .await
    else {
        return;
    };
    let Ok(z) = dir
        .progress(a.id, a.revision, Progress::Completed, Children::Leave, d18)
        .await
    else {
        return;
    };
    cx.ensure(
        z.progress == Progress::Completed,
        "progress: the last occurrence of a Count series finishes the series",
    );
}

/// `ThisAndFollowing`: at the first occurrence it is `Whole`; later it splits (new uid, First/Next), and the trash
/// of the following gives back rules and exceptions, but only if nobody changed the series since.
async fn split_rules(cx: &mut Cx<'_>, list: ObjectId) {
    let dir = cx.dir;
    let Some(t) = series(cx, list, "Split", 5).await else {
        return;
    };
    let (Some(first), Some(third)) = (following("2026-11-16"), following("2026-11-18")) else {
        return;
    };
    let patch = TaskPatch {
        title: Some("Split!".into()),
        ..Default::default()
    };
    let whole = dir.update(t.id, t.revision, patch.clone(), first).await;
    cx.ensure(
        whole
            .as_ref()
            .is_ok_and(|w| w.id == t.id && w.title == "Split!"),
        "update: ThisAndFollowing at the first occurrence is Whole",
    );
    let Ok(t) = whole else { return };
    let Ok(tail) = dir
        .update(
            t.id,
            t.revision,
            TaskPatch {
                title: Some("Tail".into()),
                ..Default::default()
            },
            third,
        )
        .await
    else {
        cx.ensure(false, "update: ThisAndFollowing splits the series");
        return;
    };
    cx.ensure(
        tail.id != t.id && tail.uid != t.uid && tail.title == "Tail",
        "split: a new task with a new uid",
    );
    cx.ensure(
        tail.due == on("2026-11-18"),
        "split: the new task starts at the occurrence",
    );
    cx.ensure(
        tail.related
            .iter()
            .any(|r| r.uid == t.uid && r.relation == Relation::First),
        "split: the new task is related First to the old",
    );
    let head = cx.fresh(t.id).await;
    cx.ensure(
        head.is_some_and(|h| {
            h.related
                .iter()
                .any(|r| r.uid == tail.uid && r.relation == Relation::Next)
        }),
        "split: the old task is related Next to the new one",
    );
    let Some(t2) = series(cx, list, "Cut", 5).await else {
        return;
    };
    let Ok(cut) = dir.delete(t2.id, t2.revision, third).await else {
        cx.ensure(false, "delete: ThisAndFollowing shortens the series");
        return;
    };
    let s = cx.summary(list, t2.id).await;
    cx.ensure(
        s.is_some_and(|s| s.occurrence == occurrence("2026-11-16T00:00:00")),
        "delete: ThisAndFollowing keeps the earlier occurrences",
    );
    let back = dir.restore(t2.id, cut.scope).await;
    cx.ensure(
        back.is_ok(),
        "restore: ThisAndFollowing gives the series back",
    );
    let Some(t3) = series(cx, list, "Cut2", 5).await else {
        return;
    };
    let Ok(cut) = dir.delete(t3.id, t3.revision, third).await else {
        return;
    };
    let Some(now) = cx.fresh(t3.id).await else {
        return;
    };
    let edit = TaskPatch {
        title: Some("Meanwhile".into()),
        ..Default::default()
    };
    let _ = dir
        .update(now.id, now.revision, edit, SeriesScope::Whole)
        .await;
    let late = dir.restore(t3.id, cut.scope).await;
    cx.fails(
        &late,
        "restore: ThisAndFollowing after the series changed is Changed",
        is_changed,
    );
}

/// A rule outside the subset stays as text; the first occurrence is visible; editing one occurrence is Unsupported.
async fn outside_subset(cx: &mut Cx<'_>, list: ObjectId) {
    let dir = cx.dir;
    let original = vec![Recurrence::Original("FREQ=MONTHLY;BYDAY=2TU".to_owned())];
    let rest = TaskPatch {
        due: Some(on("2026-11-10")),
        recurrence: Some(original.clone()),
        ..Default::default()
    };
    let Some(t) = cx.task_with(list, "Odd rule", rest).await else {
        return;
    };
    cx.ensure(
        t.recurrence == original,
        "create: a rule outside the subset is kept as text",
    );
    cx.ensure(
        cx.summary(list, t.id)
            .await
            .is_some_and(|s| s.recurring && s.due == on("2026-11-10")),
        "list: its first occurrence is visible",
    );
    let Some(scope) = this("2026-11-10") else {
        return;
    };
    let r = dir
        .update(t.id, t.revision, TaskPatch::default(), scope)
        .await;
    let ok = matches!(&r, Err(TaskError::Common(crate::tasks_v1::Error::Unsupported { capability, .. })) if capability == "task.edit");
    cx.ensure(
        ok && r.as_ref().err().is_some_and(is_unsupported),
        "update: one occurrence of such a series is Unsupported{task.edit}",
    );
}
