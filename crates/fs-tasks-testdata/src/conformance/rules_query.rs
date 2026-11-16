//! Rules for `list`: day ranges, order, filters, pages, the view zone.
//!
//! Pattern: Template method (rules as plain checks collected in the report; the seeded tasks are named T1 to T5).

use super::cx::{Cx, is_unknown_zone};
use crate::occurrence;
use crate::tasks_v1::{
    Date, DateField, Due, DueRange, ObjectId, Order, ParentFilter, Progress, TaskPatch, Zone,
};
use fs_types::BasicValue;

fn day(text: &str) -> Option<Date> {
    Date::parse(text).ok()
}

fn due(d: Due) -> TaskPatch {
    TaskPatch {
        due: Some(Some(d)),
        ..Default::default()
    }
}

fn prio(p: u8, d: Option<Due>) -> TaskPatch {
    TaskPatch {
        priority: Some(p),
        due: d.map(Some),
        ..Default::default()
    }
}

/// T1 plain date 16th (priority 3); T2 timed 23:30 New York on the 16th = 05:30 Berlin on the 17th (priority 1);
/// T3 floating 17th 10:00 (priority 0); T4 undated (priority 2); T5 timed 17th 08:00 Berlin (priority 0).
async fn seed(cx: &mut Cx<'_>, list: ObjectId) -> Option<[ObjectId; 5]> {
    let ny = Zone::iana("America/New_York").ok()?;
    let berlin = cx.berlin.clone();
    let timed = |at: &str, zone: Zone| {
        Some(Due::Timed {
            at: occurrence(at)?,
            zone,
            zone_assumed: false,
        })
    };
    let t1 = cx
        .task_with(
            list,
            "T1 Alpha",
            prio(3, Some(Due::Date(day("2026-11-16")?))),
        )
        .await?;
    let t2 = cx
        .task_with(list, "T2 beta", prio(1, timed("2026-11-16T23:30:00", ny)))
        .await?;
    let t3 = cx
        .task_with(
            list,
            "T3 Gamma",
            due(Due::Floating(occurrence("2026-11-17T10:00:00")?)),
        )
        .await?;
    let t4 = cx.task_with(list, "T4 delta", prio(2, None)).await?;
    let t5 = cx
        .task_with(
            list,
            "T5 Epsilon",
            due(timed("2026-11-17T08:00:00", berlin)?),
        )
        .await?;
    Some([t1.id, t2.id, t3.id, t4.id, t5.id])
}

pub(super) async fn rules(cx: &mut Cx<'_>) {
    let dir = cx.dir;
    let Some(list) = cx.list("query").await else {
        return;
    };
    let Some([t1, t2, t3, t4, t5]) = seed(cx, list.id).await else {
        return;
    };
    let Some(l) = cx.list("query other").await else {
        return;
    };
    let _ = cx.task(l.id, "T6 outside").await;
    let range = |from: Option<&str>, to: Option<&str>| DueRange {
        field: DateField::Due,
        from: from.and_then(day),
        to: to.and_then(day),
    };
    let mut q = cx.query(&[list.id]);
    q.due = Some(range(Some("2026-11-17"), Some("2026-11-17")));
    let mut got = cx.ids(&q).await;
    got.sort();
    let mut want = [t2, t3, t5];
    want.sort();
    cx.ensure(got == want, "list: a day range counts a timed due by its day in the view zone, plain and floating by their own day");
    q.due = Some(range(None, Some("2026-11-16")));
    cx.ensure(
        cx.ids(&q).await == [t1],
        "list: Heute = to today, from open (an undated task is not due)",
    );
    q.due = None;
    q.undated = Some(true);
    cx.ensure(cx.ids(&q).await == [t4], "list: undated = Some(true)");
    q.undated = Some(false);
    cx.ensure(cx.ids(&q).await.len() == 4, "list: undated = Some(false)");
    q.undated = None;
    q.order = Order::Due;
    cx.ensure(
        cx.ids(&q).await == [t1, t2, t5, t3, t4],
        "list: Due by day, timed before floating on a day, undated last",
    );
    q.order = Order::Priority;
    let mut zero = [t3, t5];
    zero.sort();
    cx.ensure(
        cx.ids(&q).await == [t2, t4, t1, zero[0], zero[1]],
        "list: Priority 1 first, 0 (not set) last, ties by id",
    );
    q.order = Order::Rank;
    cx.ensure(
        cx.ids(&q).await == [t5, t4, t3, t2, t1],
        "list: Rank = newest on top",
    );
    filters(cx, list.id, [t1, t2, t3, t4, t5]).await;
    let mut q = cx.query(&[list.id]);
    q.view_zone = match Zone::custom("/Custom/Zzconf") {
        Ok(z) => z,
        Err(_) => return,
    };
    cx.fails(
        &dir.list(&q, None, 10).await,
        "list: a custom view zone is UnknownZone",
        is_unknown_zone,
    );
}

async fn filters(cx: &mut Cx<'_>, list: ObjectId, [t1, t2, t3, t4, t5]: [ObjectId; 5]) {
    let dir = cx.dir;
    let mut q = cx.query(&[list]);
    q.text = Some("EPSILON".into());
    cx.ensure(
        cx.ids(&q).await == [t5],
        "list: text is found in the title, whatever the case",
    );
    q.text = None;
    if cx.needs(
        "list: Finished order and the progress filter",
        &["task.progress"],
    ) {
        let Some(done) = cx.fresh(t4).await else {
            return;
        };
        let _ = cx.set_progress(&done, Progress::Completed).await;
        let Some(done2) = cx.fresh(t2).await else {
            return;
        };
        let _ = cx.set_progress(&done2, Progress::Completed).await;
        q.progress = vec![Progress::Completed];
        q.order = Order::Finished;
        cx.ensure(
            cx.ids(&q).await == [t2, t4],
            "list: Finished = last finished first",
        );
        q.progress = vec![Progress::NeedsAction];
        q.order = Order::Updated;
        let open = cx.ids(&q).await;
        cx.ensure(
            open.len() == 3 && !open.contains(&t2) && !open.contains(&t4),
            "list: progress filter",
        );
    }
    q.order = Order::Updated;
    let Some(three) = cx.fresh(t3).await else {
        return;
    };
    let patch = TaskPatch {
        labels: Some(vec!["k".into(), "m".into()]),
        ..Default::default()
    };
    let _ = dir
        .update(
            three.id,
            three.revision,
            patch,
            crate::tasks_v1::SeriesScope::Whole,
        )
        .await;
    q.progress = Vec::new();
    cx.ensure(
        cx.ids(&q).await.first() == Some(&t3),
        "list: Updated = last written first",
    );
    q.labels = vec!["k".into(), "m".into()];
    cx.ensure(
        cx.ids(&q).await == [t3],
        "list: all named labels must be set",
    );
    q.labels = vec!["k".into(), "x".into()];
    cx.ensure(
        cx.ids(&q).await.is_empty(),
        "list: one missing label leaves the task out",
    );
    q.labels = Vec::new();
    let Some(p) = cx.fresh(t1).await else { return };
    let kid = TaskPatch {
        parent: Some(Some(p.id)),
        ..Default::default()
    };
    let kid = cx.task_with(list, "Kid of T1", kid).await;
    q.parent = Some(ParentFilter::TopLevel);
    cx.ensure(
        cx.ids(&q).await.len() == 5,
        "list: TopLevel leaves subtasks out",
    );
    q.parent = Some(ParentFilter::Of(t1));
    cx.ensure(
        cx.ids(&q).await == kid.map(|k| k.id).into_iter().collect::<Vec<_>>(),
        "list: the subtasks of one task",
    );
    q.parent = None;
    q.order = Order::Rank;
    pages(cx, &q).await;
}

/// Pages: `next` while more follow, together they are the whole list; size 0 is an empty page with `next`.
async fn pages(cx: &mut Cx<'_>, q: &crate::tasks_v1::TaskQuery) {
    let dir = cx.dir;
    let all = cx.ids(q).await;
    let mut seen = Vec::new();
    let mut after = None;
    for _ in 0..10 {
        let Ok(page) = dir.list(q, after.as_ref(), 2).await else {
            break;
        };
        seen.extend(page.items.iter().map(|s| s.id));
        match page.next {
            Some(n) => after = Some(n),
            None => break,
        }
    }
    cx.ensure(
        seen == all && all.len() > 2,
        "list: pages of 2 together are the whole list, in the same order",
    );
    let none = dir.list(q, None, 0).await;
    cx.ensure(
        none.is_ok_and(|p| p.items.is_empty() && p.next.is_some()),
        "list: size 0 is an empty page with next",
    );
}
