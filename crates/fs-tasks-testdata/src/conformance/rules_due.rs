//! Rules for the day range of `list` in more than one view zone: the edge of a day (the last second and the first), a zone
//! east and one west of the date line's neighbours, the field `Start` and the earlier day of `DueOrStart`.
//!
//! Pattern: Template method (rules as plain checks collected in the report; the seeded tasks are named A to F).

use super::cx::Cx;
use crate::occurrence;
use crate::tasks_v1::{
    Date, DateField, Due, DueRange, ObjectId, Order, TaskPatch, TaskQuery, Zone,
};
use fs_types::BasicValue;

fn day(text: &str) -> Option<Date> {
    Date::parse(text).ok()
}

fn timed(at: &str, zone: &Zone) -> Option<Due> {
    Some(Due::Timed {
        at: occurrence(at)?,
        zone: zone.clone(),
        zone_assumed: false,
    })
}

struct Seed {
    a: ObjectId,
    b: ObjectId,
    c: ObjectId,
    d: ObjectId,
    e: ObjectId,
}

/// A: due 16th 23:59:59 Berlin; B: due 17th 00:00:00 Berlin; C: due plain 17th; D: start plain 10th and due plain 20th;
/// E: start 18th 23:30 UTC only; F: undated.
async fn seed(cx: &mut Cx<'_>, list: ObjectId) -> Option<Seed> {
    let berlin = cx.berlin.clone();
    let utc = Zone::iana("UTC").ok()?;
    let with = |due: Option<Due>, start: Option<Due>| TaskPatch {
        due: due.map(Some),
        start: start.map(Some),
        ..TaskPatch::default()
    };
    let a = cx
        .task_with(list, "A", with(timed("2026-11-16T23:59:59", &berlin), None))
        .await?;
    let b = cx
        .task_with(list, "B", with(timed("2026-11-17T00:00:00", &berlin), None))
        .await?;
    let c = cx
        .task_with(list, "C", with(Some(Due::Date(day("2026-11-17")?)), None))
        .await?;
    let d = cx
        .task_with(
            list,
            "D",
            with(
                Some(Due::Date(day("2026-11-20")?)),
                Some(Due::Date(day("2026-11-10")?)),
            ),
        )
        .await?;
    let e = cx
        .task_with(list, "E", with(None, timed("2026-11-18T23:30:00", &utc)))
        .await?;
    cx.task(list, "F").await?;
    Some(Seed {
        a: a.id,
        b: b.id,
        c: c.id,
        d: d.id,
        e: e.id,
    })
}

async fn got(cx: &Cx<'_>, q: &TaskQuery) -> Vec<ObjectId> {
    let mut ids = cx.ids(q).await;
    ids.sort();
    ids
}

fn sorted<const N: usize>(mut v: [ObjectId; N]) -> Vec<ObjectId> {
    v.sort();
    v.to_vec()
}

pub(super) async fn rules(cx: &mut Cx<'_>) {
    use DateField::{Due as D, DueOrStart as DS, Start as S};
    let Some(list) = cx.list("due").await else {
        return;
    };
    let Some(s) = seed(cx, list.id).await else {
        return;
    };
    let zone = |id: &str| Zone::iana(id).ok();
    let (Some(utc), Some(east), Some(west)) = (
        zone("UTC"),
        zone("Pacific/Kiritimati"),
        zone("Pacific/Pago_Pago"),
    ) else {
        cx.ensure(false, "setup: the zones UTC, Kiritimati and Pago Pago");
        return;
    };
    let berlin = cx.berlin.clone();
    let mut q = cx.query(&[list.id]);
    q.order = Order::Rank;
    let mut check = |field: DateField, from: Option<&str>, to: Option<&str>, view: &Zone| {
        q.due = Some(DueRange {
            field,
            from: from.and_then(day),
            to: to.and_then(day),
        });
        q.view_zone = view.clone();
        q.clone()
    };
    let cases: Vec<(TaskQuery, Vec<ObjectId>, &str)> = vec![
        (
            check(D, Some("2026-11-17"), Some("2026-11-17"), &berlin),
            sorted([s.b, s.c]),
            "list: the first second of a day (00:00:00 in the view zone) is on that day",
        ),
        (
            check(D, Some("2026-11-16"), Some("2026-11-16"), &berlin),
            sorted([s.a]),
            "list: the last second of a day (23:59:59 in the view zone) is on that day and the next second is not",
        ),
        (
            check(D, None, Some("2026-11-16"), &berlin),
            sorted([s.a]),
            "list: no start of the range, a plain 17th is not before the 16th",
        ),
        (
            check(D, Some("2026-11-17"), None, &berlin),
            sorted([s.b, s.c, s.d]),
            "list: no end of the range",
        ),
        (
            check(D, Some("2026-11-18"), Some("2026-11-19"), &berlin),
            Vec::new(),
            "list: a range with no task is empty",
        ),
        (
            check(D, Some("2026-11-16"), Some("2026-11-16"), &utc),
            sorted([s.a, s.b]),
            "list: in UTC both timed values are on the 16th, the plain 17th is not",
        ),
        (
            check(D, Some("2026-11-17"), Some("2026-11-17"), &utc),
            sorted([s.c]),
            "list: in UTC only the plain 17th is on the 17th",
        ),
        (
            check(D, Some("2026-11-17"), Some("2026-11-17"), &east),
            sorted([s.a, s.b, s.c]),
            "list: a zone far to the east sees both timed values on the 17th",
        ),
        (
            check(D, Some("2026-11-16"), Some("2026-11-16"), &west),
            sorted([s.a, s.b]),
            "list: a zone far to the west sees both timed values on the 16th",
        ),
        (
            check(S, Some("2026-11-10"), Some("2026-11-10"), &berlin),
            sorted([s.d]),
            "list: the field Start asks the start day",
        ),
        (
            check(S, Some("2026-11-19"), Some("2026-11-19"), &berlin),
            sorted([s.e]),
            "list: a timed start counts by its day in the view zone (23:30 UTC is the 19th in Berlin)",
        ),
        (
            check(S, Some("2026-11-18"), Some("2026-11-18"), &utc),
            sorted([s.e]),
            "list: the same start is the 18th in UTC",
        ),
        (
            check(DS, Some("2026-11-10"), Some("2026-11-10"), &berlin),
            sorted([s.d]),
            "list: DueOrStart counts the earlier of the two days",
        ),
        (
            check(DS, Some("2026-11-20"), Some("2026-11-20"), &berlin),
            Vec::new(),
            "list: DueOrStart does not count the later day (the due 20th of a start on the 10th)",
        ),
        (
            check(DS, None, Some("2026-11-16"), &berlin),
            sorted([s.a, s.d]),
            "list: DueOrStart up to a day, undated tasks stay out",
        ),
        (
            check(DS, Some("2026-11-19"), Some("2026-11-19"), &berlin),
            sorted([s.e]),
            "list: DueOrStart with only a start",
        ),
    ];
    for (query, want, rule) in cases {
        let found = got(cx, &query).await;
        cx.ensure(found == want, rule);
    }
}
