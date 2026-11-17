//! Rules for tasks: create, read, update (compare-and-set), progress and subtasks.
//!
//! Pattern: Template method (rules as plain checks collected in the report).

use super::cx::{Cx, draft, is_changed, is_invalid, is_not_found, is_unknown_zone};
use crate::tasks_v1::{Due, Progress, SeriesScope, TaskPatch, Zone};

pub(super) async fn rules(cx: &mut Cx<'_>) {
    let dir = cx.dir;
    let Some(list) = cx.list("tasks").await else {
        return;
    };
    let Some(other) = cx.list("tasks other").await else {
        return;
    };
    let Some(t) = cx.task(list.id, "First").await else {
        return;
    };
    cx.ensure(
        t.revision == crate::tasks_v1::Revision(1),
        "create: revision 1",
    );
    cx.ensure(
        !t.uid.is_empty() && t.title == "First" && t.progress == Progress::NeedsAction,
        "create: uid, title, progress",
    );
    cx.ensure(
        dir.get(t.id).await.ok().flatten().as_ref() == Some(&t),
        "get: returns what create returned",
    );
    cx.ensure(
        dir.get_by_uid(&t.uid)
            .await
            .ok()
            .flatten()
            .is_some_and(|x| x.id == t.id),
        "get_by_uid: finds it",
    );
    cx.ensure(
        dir.get_by_uid("urn:zzconf:none")
            .await
            .ok()
            .flatten()
            .is_none(),
        "get_by_uid: unknown is None",
    );

    let mut same = draft(list.id, "Twin", TaskPatch::default());
    same.uid = Some(t.uid.clone());
    cx.fails(
        &dir.create(same).await,
        "create: an existing uid is Invalid",
        is_invalid,
    );
    let twice = TaskPatch {
        title: Some("x".into()),
        ..Default::default()
    };
    cx.fails(
        &dir.create(draft(list.id, "T", twice)).await,
        "create: rest.title set is Invalid",
        is_invalid,
    );
    let twice = TaskPatch {
        list: Some(other.id),
        ..Default::default()
    };
    cx.fails(
        &dir.create(draft(list.id, "T", twice)).await,
        "create: rest.list set is Invalid",
        is_invalid,
    );
    cx.fails(
        &dir.create(draft(list.id, " ", TaskPatch::default())).await,
        "create: an empty title is Invalid",
        is_invalid,
    );
    let nowhere = crate::ids::object_id(u64::MAX);
    cx.fails(
        &dir.create(draft(nowhere, "T", TaskPatch::default())).await,
        "create: an unknown list is NotFound",
        is_not_found,
    );
    update_rules(cx, &t, other.id).await;
    super::rules_subtasks::rules(cx, list.id).await;
    super::rules_subtasks::depth(cx, list.id).await;
    super::rules_subtasks::restore_depth(cx, list.id).await;
}

async fn update_rules(
    cx: &mut Cx<'_>,
    t: &crate::tasks_v1::Task,
    other: crate::tasks_v1::ObjectId,
) {
    let dir = cx.dir;
    let patch = TaskPatch {
        title: Some("Renamed".into()),
        priority: Some(2),
        labels: Some(vec!["b".into(), "a".into(), "b".into()]),
        ..Default::default()
    };
    let Ok(done) = dir
        .update(t.id, t.revision, patch.clone(), SeriesScope::Whole)
        .await
    else {
        cx.ensure(false, "update: applies the patch");
        return;
    };
    let mut labels = done.labels.clone();
    labels.sort();
    cx.ensure(
        done.title == "Renamed"
            && done.priority == 2
            && labels == ["a", "b"]
            && done.revision > t.revision,
        "update: fields set, labels are a set, revision up",
    );
    let stale = dir
        .update(t.id, t.revision, patch, SeriesScope::Whole)
        .await;
    cx.fails(&stale, "update: a stale revision is Changed", is_changed);
    let still = dir.get(t.id).await.ok().flatten();
    cx.ensure(
        still.is_some_and(|x| x.title == "Renamed" && x.revision == done.revision),
        "update: Changed writes nothing",
    );
    let bad = |p: TaskPatch| dir.update(t.id, done.revision, p, SeriesScope::Whole);
    cx.fails(
        &bad(TaskPatch {
            priority: Some(10),
            ..Default::default()
        })
        .await,
        "update: priority 10 is Invalid",
        is_invalid,
    );
    cx.fails(
        &bad(TaskPatch {
            percent_complete: Some(Some(101)),
            ..Default::default()
        })
        .await,
        "update: 101 percent is Invalid",
        is_invalid,
    );
    cx.fails(
        &bad(TaskPatch {
            title: Some(String::new()),
            ..Default::default()
        })
        .await,
        "update: an empty title is Invalid",
        is_invalid,
    );
    let at = crate::occurrence("2026-11-16T10:00:00");
    if let (Ok(custom), Some(at)) = (Zone::custom("/Custom/Zzconf"), at) {
        let due = Due::Timed {
            at,
            zone: custom,
            zone_assumed: false,
        };
        let p = TaskPatch {
            due: Some(Some(due)),
            ..Default::default()
        };
        cx.fails(
            &bad(p).await,
            "update: a zone nobody knows is UnknownZone",
            is_unknown_zone,
        );
    }
    if let Some(at) = at {
        let scope = SeriesScope::This { recurrence_id: at };
        let r = dir
            .update(t.id, done.revision, TaskPatch::default(), scope)
            .await;
        cx.ensure(
            r.is_err(),
            "update: This on a task that is no series is refused",
        );
    }
    let moved = dir
        .update(
            t.id,
            done.revision,
            TaskPatch {
                list: Some(other),
                ..Default::default()
            },
            SeriesScope::Whole,
        )
        .await;
    cx.ensure(
        moved.as_ref().is_ok_and(|m| m.list == other),
        "update: moves the task to another list",
    );
}
