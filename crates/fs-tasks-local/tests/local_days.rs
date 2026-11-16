//! The day range of `list` is decided on the derived day columns in SQL. It must give the answer of `zones::day_in` on every
//! task (the rule it replaced): held against that rule over many values, zones and ranges, including the hours a zone skips
//! or repeats and a day a zone does not have; and a file made before the columns existed gets them in step 6.
//!
//! Pattern: Differential test (the new implementation against the old rule as an oracle) and a fixture test of a step.

#[allow(dead_code)]
mod common;

use std::sync::Arc;
use std::sync::atomic::AtomicI64;

use chrono::NaiveDate;
use common::{LocalFx, draft};
use fs_objects::v5::codec::id_value;
use fs_objects::v5::envelope::insert;
use fs_objects::v5::spec::Table;
use fs_objects::v5::{Step, Store, new_id};
use fs_tasks_local::registry::DOMAIN;
use fs_tasks_local::stored::row_values;
use fs_tasks_local::tables::{
    CHECKLIST, CHECKLIST_SEQ, GRANT, LABEL, LIST, STEP_1, STEP_2, STEP_3, STEP_4, TASK,
};
use fs_tasks_local::zones::day_in;
use fs_tasks_local::{Clock, LocalTasks};
use fs_tasks_testdata::tasks_v1::{
    Date, DateField, Due, DueRange, ObjectId, Order, TaskPatch, TaskQuery, Tasks, Trash, Utc, Zone,
};
use fs_tasks_testdata::{START_MS, blank_task, occurrence};
use fs_types::BasicValue;

fn date(s: &str) -> Date {
    Date::parse(s).unwrap()
}

fn timed(at: &str, zone: &str) -> Due {
    Due::Timed {
        at: occurrence(at).unwrap(),
        zone: Zone::iana(zone).unwrap(),
        zone_assumed: false,
    }
}

fn query(list: ObjectId, view: &str, due: Option<DueRange>) -> TaskQuery {
    TaskQuery {
        lists: vec![list],
        text: None,
        progress: Vec::new(),
        due,
        undated: None,
        assignee: None,
        labels: Vec::new(),
        parent: None,
        ready: None,
        order: Order::Rank,
        view_zone: Zone::iana(view).unwrap(),
        trash: Trash::Exclude,
    }
}

/// Due and start values around the days that are hard: midnight, the last second, an hour a zone skips (Berlin, 29 March
/// 2026, 02:30 does not exist) and repeats (25 October, 02:30), a day a zone does not have (Samoa, 30 December 2011).
fn values() -> Vec<(Option<Due>, Option<Due>)> {
    let mut out = Vec::new();
    for (at, zone) in [
        ("2026-11-16T23:59:59", "Europe/Berlin"),
        ("2026-11-17T00:00:00", "Europe/Berlin"),
        ("2026-03-29T01:59:59", "Europe/Berlin"),
        ("2026-03-29T02:30:00", "Europe/Berlin"),
        ("2026-03-29T03:00:00", "Europe/Berlin"),
        ("2026-10-25T02:30:00", "Europe/Berlin"),
        ("2026-10-24T23:59:59", "America/New_York"),
        ("2026-10-25T00:00:00", "America/New_York"),
        ("2011-12-29T23:30:00", "Pacific/Apia"),
        ("2011-12-31T00:00:00", "Pacific/Apia"),
        ("2026-10-25T13:00:00", "Pacific/Kiritimati"),
        ("2026-10-25T10:59:59", "Pacific/Pago_Pago"),
        ("2026-10-25T23:30:00", "UTC"),
        ("2026-10-26T00:00:00", "UTC"),
    ] {
        out.push((Some(timed(at, zone)), None));
        out.push((None, Some(timed(at, zone))));
    }
    for d in [
        "2026-03-28",
        "2026-03-29",
        "2026-10-25",
        "2011-12-30",
        "2026-11-17",
    ] {
        out.push((Some(Due::Date(date(d))), None));
        out.push((
            Some(Due::Date(date(d))),
            Some(timed("2026-03-29T03:00:00", "Europe/Berlin")),
        ));
    }
    out.push((
        Some(Due::Floating(occurrence("2026-10-25T23:30:00").unwrap())),
        Some(Due::Date(date("2026-10-20"))),
    ));
    out.push((None, None));
    out
}

fn expected(
    t: &(Option<Due>, Option<Due>),
    field: DateField,
    from: Option<Date>,
    to: Option<Date>,
    view: chrono_tz::Tz,
) -> bool {
    let day = |d: &Option<Due>| d.as_ref().and_then(|d| day_in(d, view));
    let days: Vec<NaiveDate> = match field {
        DateField::Due => day(&t.0).into_iter().collect(),
        DateField::Start => day(&t.1).into_iter().collect(),
        DateField::DueOrStart => day(&t.0)
            .into_iter()
            .chain(day(&t.1))
            .min()
            .into_iter()
            .collect(),
    };
    days.iter()
        .any(|d| from.is_none_or(|f| *d >= f.as_naive()) && to.is_none_or(|e| *d <= e.as_naive()))
}

#[tokio::test]
async fn the_day_range_in_sql_gives_the_answer_of_the_rule_it_replaced() {
    let fx = LocalFx::memory().await;
    let list = fx.tasks.create_list("L", None, None).await.unwrap().id;
    let all = values();
    let mut made = Vec::new();
    for (i, (due, start)) in all.iter().enumerate() {
        let rest = TaskPatch {
            due: due.clone().map(Some),
            start: start.clone().map(Some),
            ..TaskPatch::default()
        };
        made.push(
            fx.tasks
                .create(draft(list, &format!("t{i}"), rest))
                .await
                .unwrap()
                .id,
        );
    }
    let days = [
        None,
        Some("2011-12-29"),
        Some("2011-12-30"),
        Some("2011-12-31"),
        Some("2026-03-28"),
        Some("2026-03-29"),
        Some("2026-10-24"),
        Some("2026-10-25"),
        Some("2026-10-26"),
        Some("2026-11-16"),
        Some("2026-11-17"),
    ];
    let mut compared = 0;
    for view in [
        "UTC",
        "Europe/Berlin",
        "Pacific/Kiritimati",
        "Pacific/Pago_Pago",
        "America/New_York",
        "Pacific/Apia",
        "Australia/Lord_Howe",
    ] {
        let tz: chrono_tz::Tz = view.parse().unwrap();
        for field in [DateField::Due, DateField::Start, DateField::DueOrStart] {
            for from in days {
                for to in days {
                    let (f, t) = (from.map(date), to.map(date));
                    let range = DueRange {
                        field,
                        from: f,
                        to: t,
                    };
                    let page = fx
                        .tasks
                        .list(&query(list, view, Some(range)), None, 1000)
                        .await
                        .unwrap();
                    let mut got: Vec<ObjectId> = page.items.iter().map(|s| s.id).collect();
                    got.sort();
                    let mut want: Vec<ObjectId> = all
                        .iter()
                        .zip(&made)
                        .filter(|(v, _)| expected(v, field, f, t, tz))
                        .map(|(_, id)| *id)
                        .collect();
                    want.sort();
                    assert_eq!(got, want, "{view} {field:?} {from:?}..{to:?}");
                    compared += 1;
                }
            }
        }
    }
    assert_eq!(compared, 7 * 3 * 11 * 11);
}

/// A changed due value moves the task in the range, and a task without one is on no day.
#[tokio::test]
async fn an_update_of_the_due_value_moves_the_day_columns_with_it() {
    let fx = LocalFx::memory().await;
    let list = fx.tasks.create_list("L", None, None).await.unwrap().id;
    let t = fx
        .tasks
        .create(draft(list, "t", TaskPatch::default()))
        .await
        .unwrap();
    let on = |d: &str| {
        Some(DueRange {
            field: DateField::Due,
            from: Some(date(d)),
            to: Some(date(d)),
        })
    };
    let count = |q: TaskQuery| {
        let fx = &fx;
        async move { fx.tasks.list(&q, None, 10).await.unwrap().items.len() }
    };
    assert_eq!(count(query(list, "UTC", on("2026-11-17"))).await, 0);
    let rest = |d: Option<Due>| TaskPatch {
        due: Some(d),
        ..TaskPatch::default()
    };
    let t = fx
        .tasks
        .update(
            t.id,
            t.revision,
            rest(Some(Due::Date(date("2026-11-17")))),
            fs_tasks_testdata::tasks_v1::SeriesScope::Whole,
        )
        .await
        .unwrap();
    assert_eq!(count(query(list, "UTC", on("2026-11-17"))).await, 1);
    let t = fx
        .tasks
        .update(
            t.id,
            t.revision,
            rest(Some(Due::Date(date("2026-11-18")))),
            fs_tasks_testdata::tasks_v1::SeriesScope::Whole,
        )
        .await
        .unwrap();
    assert_eq!(count(query(list, "UTC", on("2026-11-17"))).await, 0);
    assert_eq!(count(query(list, "UTC", on("2026-11-18"))).await, 1);
    fx.tasks
        .update(
            t.id,
            t.revision,
            rest(None),
            fs_tasks_testdata::tasks_v1::SeriesScope::Whole,
        )
        .await
        .unwrap();
    assert_eq!(count(query(list, "UTC", on("2026-11-18"))).await, 0);
}

static OLD_TABLES: [Table; 6] = [LIST, TASK, LABEL, GRANT, CHECKLIST, CHECKLIST_SEQ];
static OLD_STEPS: [Step; 4] = [
    Step::Create(&STEP_1),
    Step::Create(&STEP_2),
    Step::Create(&STEP_3),
    Step::Create(&STEP_4),
];

/// A file with the four steps of run 2a (the grant table as it is now) and a task with a due value: opened by the current
/// provider it gets the day columns (step 6), and the range finds the task.
#[tokio::test]
async fn a_file_made_before_the_day_columns_gets_them_in_step_6() {
    let dir = format!("{}/steps", env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = format!("{dir}/before_days.db");
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{path}{ext}"));
    }
    let who = common::known(1);
    let (list, task) = {
        let old = fs_objects::v5::registry::Domain {
            tables: &OLD_TABLES,
            steps: &OLD_STEPS,
            ..DOMAIN
        };
        let store = Store::open(&path, START_MS).await.unwrap();
        store.register_domain(&old, START_MS).await.unwrap();
        let c = store.begin().await.unwrap();
        let list = new_id(START_MS).unwrap();
        insert(&c, list, "list", "L", &who, START_MS).await.unwrap();
        let [oid, oart, oabzug] = fs_objects::v5::codec::person_values(&who);
        c.insert(
            &LIST,
            vec![
                id_value(list),
                fs_objects::v5::codec::text("L"),
                fs_objects::v5::codec::opt_text(None),
                oid,
                oart,
                oabzug,
            ],
        )
        .await
        .unwrap();
        let task = new_id(START_MS + 1).unwrap();
        insert(&c, task, "task", "T", &who, START_MS + 1)
            .await
            .unwrap();
        let mut t = blank_task(
            task,
            list,
            "urn:uuid:before",
            "T",
            Utc::from_millis(START_MS + 1),
        );
        t.due = Some(timed("2026-11-17T00:30:00", "Europe/Berlin"));
        c.insert(&TASK, row_values(&t, None).unwrap())
            .await
            .unwrap();
        c.finish(Ok(())).await.unwrap();
        (list, task)
    };
    let now = Arc::new(AtomicI64::new(START_MS + 10));
    let tasks = LocalTasks::open(&path, who, Clock::from_counter(now))
        .await
        .expect("steps 5 and 6 are applied to the old file");
    let on = |d: &str, view: &str| {
        query(
            list,
            view,
            Some(DueRange {
                field: DateField::Due,
                from: Some(date(d)),
                to: Some(date(d)),
            }),
        )
    };
    let found = |q: TaskQuery| {
        let tasks = &tasks;
        async move {
            tasks
                .list(&q, None, 10)
                .await
                .unwrap()
                .items
                .iter()
                .map(|s| s.id)
                .collect::<Vec<_>>()
        }
    };
    assert_eq!(found(on("2026-11-17", "Europe/Berlin")).await, [task]);
    assert_eq!(
        found(on("2026-11-16", "UTC")).await,
        [task],
        "23:30 UTC on the 16th"
    );
    assert!(found(on("2026-11-17", "UTC")).await.is_empty());
}
