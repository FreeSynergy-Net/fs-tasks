//! A file made before the day columns existed (the four steps of run 2a) gets them in step 6 (a code step that fills the
//! columns from the tasks), and the range of days finds its task in every zone.
//!
//! Pattern: Fixture test of a step (the old file is made with the code of the old steps, then opened by the current provider).

#[allow(dead_code)]
mod common;

use std::sync::Arc;
use std::sync::atomic::AtomicI64;

use common::{date, day_query, timed};
use fs_objects::v5::codec::{id_value, opt_text, person_values, text};
use fs_objects::v5::envelope::insert;
use fs_objects::v5::spec::Table;
use fs_objects::v5::{Step, Store, new_id};
use fs_tasks_local::registry::DOMAIN;
use fs_tasks_local::stored::row_values;
use fs_tasks_local::tables::{
    CHECKLIST, CHECKLIST_SEQ, GRANT, LABEL, LIST, STEP_1, STEP_2, STEP_3, STEP_4, TASK,
};
use fs_tasks_local::{Clock, LocalTasks};
use fs_tasks_testdata::tasks_v1::{DateField, DueRange, Tasks, Utc};
use fs_tasks_testdata::{START_MS, blank_task};

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
        let [oid, oart, oabzug] = person_values(&who);
        c.insert(
            &LIST,
            vec![id_value(list), text("L"), opt_text(None), oid, oart, oabzug],
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
        day_query(
            list,
            view,
            Some(DueRange {
                field: DateField::Due,
                from: Some(date(d)),
                to: Some(date(d)),
            }),
        )
    };
    let found = |q: fs_tasks_testdata::tasks_v1::TaskQuery| {
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
    assert_eq!(found(on("2026-11-17", "UTC")).await.len(), 0);
}
