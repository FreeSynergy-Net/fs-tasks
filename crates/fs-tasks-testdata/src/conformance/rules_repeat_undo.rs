//! Undo, the scope questions, the claim that ends and skipping an occurrence of a series from completion.
//!
//! Pattern: Template method (rules as plain checks collected in the report).

use super::cx::{Cx, is_invalid};
use super::rules_repeat::{finish, naive_of, rule, series, timed};
use crate::tasks_v1::{
    ChecklistChange, Frequency, ObjectId, OverrideChange, Progress, Recurrence, RepeatFrom,
    RuleEnd, SeriesScope, Task, TaskDraft, TaskPatch,
};
use chrono::Duration;

/// Undo takes back only the newest exception; the ticks and the claim do not come back; a wrong scope is `Invalid`.
pub(super) async fn undo_and_scope(cx: &mut Cx<'_>, list: ObjectId) {
    let d = timed(cx, "2026-11-10T09:00:00");
    let draft = TaskDraft {
        checklist: vec!["a".into()],
        ..super::cx::draft(
            list,
            "undo",
            TaskPatch {
                due: Some(Some(d)),
                recurrence: Some(vec![Recurrence::Rule(rule(
                    Frequency::Daily,
                    1,
                    RuleEnd::Count(5),
                ))]),
                repeat_from: Some(RepeatFrom::Completion),
                ..TaskPatch::default()
            },
        )
    };
    let Ok(t) = cx.dir.create(draft).await else {
        cx.ensure(false, "setup: a series with a checklist");
        return;
    };
    let tick = ChecklistChange::Check {
        key: t.checklist[0].key.clone(),
        done: true,
    };
    let Ok(t1) = cx.dir.checklist(t.id, t.revision, tick).await else {
        return;
    };
    let Some(first) = naive_of(&t1) else { return };
    let Ok(a) = finish(cx, &t1, Progress::Completed, SeriesScope::Whole).await else {
        return;
    };
    cx.ensure(
        a.checklist.iter().all(|c| c.done.is_none()),
        "repeat: moving on takes the ticks off the checklist",
    );
    let Some(second) = naive_of(&a) else { return };
    let Ok(b) = finish(cx, &a, Progress::Completed, SeriesScope::Whole).await else {
        return;
    };
    let old = finish(
        cx,
        &b,
        Progress::NeedsAction,
        SeriesScope::This {
            recurrence_id: first,
        },
    )
    .await;
    cx.fails(
        &old,
        "repeat: undo of an older exception than the newest is Invalid",
        is_invalid,
    );
    let future = finish(
        cx,
        &b,
        Progress::Completed,
        SeriesScope::This {
            recurrence_id: first,
        },
    )
    .await;
    cx.fails(
        &future,
        "repeat: a finished state with This on another occurrence than the current one is Invalid",
        is_invalid,
    );
    let Ok(back) = finish(
        cx,
        &b,
        Progress::NeedsAction,
        SeriesScope::This {
            recurrence_id: second,
        },
    )
    .await
    else {
        cx.ensure(false, "repeat: undo of the newest exception answers");
        return;
    };
    cx.ensure(
        naive_of(&back) == Some(second)
            && back.overrides.len() == 1
            && matches!(back.recurrence.first(), Some(Recurrence::Rule(r)) if r.end == RuleEnd::Count(4))
            && back.checklist.iter().all(|c| c.done.is_none()),
        "repeat: undo sets the reference point back, removes the exception, Count + 1, and gives no ticks back",
    );
    let split = finish(
        cx,
        &back,
        Progress::Completed,
        SeriesScope::ThisAndFollowing {
            recurrence_id: first,
        },
    )
    .await;
    cx.fails(
        &split,
        "repeat: ThisAndFollowing on another occurrence than the current one is Invalid",
        is_invalid,
    );
    let rid = naive_of(&back);
    let at_current = match rid {
        Some(r) => {
            finish(
                cx,
                &back,
                Progress::Completed,
                SeriesScope::ThisAndFollowing { recurrence_id: r },
            )
            .await
        }
        None => return,
    };
    cx.ensure(
        at_current.is_ok_and(|n| n.progress == Progress::NeedsAction && n.overrides.len() == 2),
        "repeat: ThisAndFollowing on the current occurrence is the same as Whole",
    );
}

/// Moving on ends the claim (the task is free again).
pub(super) async fn moving_on(cx: &mut Cx<'_>, list: ObjectId) {
    if !cx.caps("task.claim") {
        return;
    }
    let d = timed(cx, "2026-11-10T09:00:00");
    let Some(t) = series(
        cx,
        list,
        "claimed",
        Some(d),
        None,
        rule(Frequency::Daily, 1, RuleEnd::Never),
    )
    .await
    else {
        return;
    };
    let until = crate::tasks_v1::Utc::from_millis(t.updated.millis() + 3_600_000 * 24 * 365);
    let Ok(c) = cx.dir.claim(t.id, t.revision, until).await else {
        cx.skip("repeat: moving on ends the claim (the provider refused the claim)");
        return;
    };
    let moved = finish(cx, &c, Progress::Completed, SeriesScope::Whole).await;
    cx.ensure(
        moved.is_ok_and(|m| m.claim.is_none()),
        "repeat: moving on ends the claim",
    );
}

/// `delete(This)` at the current occurrence skips it (exception `Excluded`); `restore(This)` takes the newest back.
pub(super) async fn skipping(cx: &mut Cx<'_>, list: ObjectId) {
    let d = timed(cx, "2026-11-10T09:00:00");
    let Some(t) = series(
        cx,
        list,
        "skip",
        Some(d),
        None,
        rule(Frequency::Daily, 1, RuleEnd::Never),
    )
    .await
    else {
        return;
    };
    let Some(rid) = naive_of(&t) else { return };
    let later = rid_plus(&t, 5);
    if let Some(l) = later {
        let r = cx
            .dir
            .delete(t.id, t.revision, SeriesScope::This { recurrence_id: l })
            .await;
        cx.fails(
            &r,
            "repeat: delete(This) on another occurrence than the current one is Invalid",
            is_invalid,
        );
    }
    let Ok(gone) = cx
        .dir
        .delete(t.id, t.revision, SeriesScope::This { recurrence_id: rid })
        .await
    else {
        cx.ensure(
            false,
            "repeat: delete(This) at the current occurrence answers",
        );
        return;
    };
    let Some(after) = cx.fresh(t.id).await else {
        return;
    };
    cx.ensure(
        gone.scope == (SeriesScope::This { recurrence_id: rid })
            && after.progress == Progress::NeedsAction
            && naive_of(&after) != Some(rid)
            && matches!(
                after.overrides.first().map(|o| &o.change),
                Some(OverrideChange::Excluded)
            ),
        "repeat: delete(This) skips the occurrence: moves on, exception Excluded",
    );
    let back = cx
        .dir
        .restore(t.id, SeriesScope::This { recurrence_id: rid })
        .await;
    cx.ensure(
        back.is_ok_and(|b| naive_of(&b) == Some(rid) && b.overrides.is_empty()),
        "repeat: restore(This) takes the newest skipped occurrence back",
    );
}

fn rid_plus(t: &Task, days: i64) -> Option<crate::tasks_v1::LocalDateTime> {
    let at = t.due.as_ref().and_then(crate::clock::wall)?;
    crate::clock::local(at + Duration::days(days))
}
