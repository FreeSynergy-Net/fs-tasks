//! The steps of the schema (ADR 0086): a file made with step 1 only becomes the store of six steps with the same
//! fingerprints, and every store (fresh or migrated) is read by standard SQLite and has no orphan autoindex (point 5).
//! The keys of the checklist are never given twice, and a page size that is far too big is cut, not refused.
//!
//! Pattern: Fixture test (an old file is built with the code of step 1 alone, then opened by the current provider).

#[allow(dead_code)]
mod common;

use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::AtomicI64;

use common::{LocalFx, draft};
use fs_objects::v5::codec::{get_int, get_text};
use fs_objects::v5::spec::{Col, Logical, Table};
use fs_objects::v5::tables::ART;
use fs_objects::v5::{Step, Store};
use fs_tasks_local::registry::DOMAIN;
use fs_tasks_local::tables::{STEP_1, STEP_2, STEP_3, STEP_4};
use fs_tasks_local::tables_more::STEP_5;
use fs_tasks_local::{Clock, LocalTasks};
use fs_tasks_testdata::START_MS;
use fs_tasks_testdata::tasks_v1::{
    ChecklistChange, Error, Order, TaskError, TaskPatch, TaskQuery, Tasks, Trash, Zone,
};

fn path_of(name: &str) -> String {
    let dir = format!("{}/steps", env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = format!("{dir}/{name}.db");
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{path}{ext}"));
    }
    path
}

/// Point 5 (a) and (b): no orphan autoindex, and `python3 -I` with `sqlite3` says `ok` (the `-wal` file beside it).
async fn conforms(fx: LocalFx, path: &str) {
    let c = fx.tasks.store().read().await;
    let rows = c
        .query(
            "SELECT name FROM sqlite_schema WHERE type='index' AND sql IS NULL \
             AND name NOT LIKE 'sqlite_autoindex_' || tbl_name || '_%'",
            Vec::new(),
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 0, "orphan autoindexes");
    drop(c);
    drop(fx);
    let out = Command::new("python3")
        .args([
            "-I",
            "-c",
            "import sqlite3,sys; c=sqlite3.connect(sys.argv[1]); print(c.execute('PRAGMA integrity_check').fetchall()[0][0])",
            path,
        ])
        .env("TMPDIR", env!("CARGO_TARGET_TMPDIR"))
        .output()
        .expect("python3 runs");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "ok",
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[tokio::test]
async fn a_fresh_store_has_six_steps_and_is_standard_sqlite() {
    let path = path_of("fresh");
    let now = Arc::new(AtomicI64::new(START_MS));
    let fx = LocalFx::open(&path, &now).await;
    fx.tasks.create_list("L", None, None).await.unwrap();
    conforms(fx, &path).await;
}

#[tokio::test]
async fn a_file_with_step_1_only_is_migrated_with_the_same_fingerprints() {
    let path = path_of("migrated");
    let now = Arc::new(AtomicI64::new(START_MS));
    {
        static ONE: [Step; 1] = [Step::Create(&STEP_1)];
        let old = fs_objects::v5::registry::Domain {
            tables: &STEP_1,
            steps: &ONE,
            ..DOMAIN
        };
        let store = Store::open(&path, START_MS).await.unwrap();
        store.register_domain(&old, START_MS).await.unwrap();
    }
    let fx = LocalFx::open(&path, &now).await;
    let list = fx.tasks.create_list("L", None, None).await.unwrap();
    {
        let c = fx.tasks.store().read().await;
        let rows = c
            .query(
                "SELECT seq, fingerprint FROM fs_step WHERE domain = 'tasks' ORDER BY seq",
                Vec::new(),
            )
            .await
            .unwrap();
        let mut want: Vec<String> = [
            &STEP_1[..],
            &STEP_2[..],
            &STEP_3[..],
            &STEP_4[..],
            &STEP_5[..],
        ]
        .iter()
        .map(|t| {
            let s: &'static [_] = Box::leak(t.to_vec().into_boxed_slice());
            Step::Create(s).fingerprint()
        })
        .collect();
        want.push(DOMAIN.steps[5].fingerprint());
        let got: Vec<String> = rows.iter().map(|r| get_text(r, 1).unwrap()).collect();
        assert_eq!(rows.len(), 6);
        assert_eq!(get_int(&rows[0], 0).unwrap(), 1);
        assert_eq!(got, want, "the fingerprints of the six steps");
    }
    fx.tasks
        .create(draft(list.id, "T", TaskPatch::default()))
        .await
        .unwrap();
    conforms(fx, &path).await;
}

#[tokio::test]
async fn a_key_of_a_removed_item_is_not_given_again() {
    let fx = LocalFx::memory().await;
    let list = fx.tasks.create_list("L", None, None).await.unwrap();
    let mut d = draft(list.id, "T", TaskPatch::default());
    d.checklist = vec!["a".into(), "b".into(), "c".into()];
    let t = fx.tasks.create(d).await.unwrap();
    let gone_key = t.checklist.last().unwrap().key.clone();
    let t = fx
        .tasks
        .checklist(
            t.id,
            t.revision,
            ChecklistChange::Remove {
                key: gone_key.clone(),
            },
        )
        .await
        .unwrap();
    let t = fx
        .tasks
        .checklist(
            t.id,
            t.revision,
            ChecklistChange::Add {
                title: "d".into(),
                after: None,
            },
        )
        .await
        .unwrap();
    assert!(
        t.checklist.iter().all(|i| i.key != gone_key),
        "{:?}",
        t.checklist
    );
}

#[tokio::test]
async fn a_page_size_that_is_far_too_big_is_cut() {
    let fx = LocalFx::memory().await;
    let list = fx.tasks.create_list("L", None, None).await.unwrap();
    fx.tasks
        .create(draft(list.id, "T", TaskPatch::default()))
        .await
        .unwrap();
    let q = TaskQuery {
        lists: vec![list.id],
        text: None,
        progress: Vec::new(),
        due: None,
        undated: None,
        assignee: None,
        labels: Vec::new(),
        parent: None,
        ready: None,
        order: Order::Rank,
        view_zone: Zone::iana("UTC").unwrap(),
        trash: Trash::Exclude,
    };
    let page = fx.tasks.list(&q, None, usize::MAX).await.unwrap();
    assert_eq!((page.items.len(), page.next.is_none()), (1, true));
    let labels = fx.tasks.labels(&[], None, None, usize::MAX).await.unwrap();
    assert_eq!(labels.items.len(), 0);
}

/// Step 2 of run 2a (`75a71a7`, the state of that file): the grant table knew two levels. Since then the word `manage`
/// belongs to it, which changes the `CHECK` and so the fingerprint of the step that was already applied.
static OLD_GRANT: Table = Table {
    name: "tasks_grant",
    cols: &[
        Col::req("list", Logical::Ref),
        Col::req("who_id", Logical::Iri),
        Col::req("who_art", Logical::Enum(ART)),
        Col::opt("who_abzug", Logical::Ref),
        Col::req("level", Logical::Enum(&["read", "write"])),
    ],
    constraints: &[
        "PRIMARY KEY (list, who_id)",
        "FOREIGN KEY (list) REFERENCES tasks_list(id) ON DELETE CASCADE",
    ],
    indexes: &[],
};

/// A file of run 2a (three steps, step 2 with two levels) is NOT opened by the current provider: `StepMismatch` at step 2
/// (ADR 0086, the word list of a `CHECK` is part of the fingerprint). This is known and wanted: the steps stay OPEN until
/// the first merge to `main` (decision of the strategy, 2026-11-16; no store of real data exists, the branch is not
/// published), and nothing was written away to keep such a file readable. The test says so in code: when the first
/// release freezes the steps, this test is replaced by one that opens the frozen file and migrates it.
#[tokio::test]
async fn a_file_of_run_2a_with_the_old_step_2_is_refused_until_the_first_main_merge() {
    let path = path_of("run_2a");
    {
        static THREE: [Step; 3] = [
            Step::Create(&STEP_1),
            Step::Create(std::slice::from_ref(&OLD_GRANT)),
            Step::Create(&STEP_3),
        ];
        static TABLES: [Table; 5] = [
            fs_tasks_local::tables::LIST,
            fs_tasks_local::tables::TASK,
            fs_tasks_local::tables::LABEL,
            OLD_GRANT,
            fs_tasks_local::tables::CHECKLIST,
        ];
        let old = fs_objects::v5::registry::Domain {
            tables: &TABLES,
            steps: &THREE,
            ..DOMAIN
        };
        let store = Store::open(&path, START_MS).await.unwrap();
        store.register_domain(&old, START_MS).await.unwrap();
    }
    let now = Arc::new(AtomicI64::new(START_MS));
    let opened = LocalTasks::open(&path, common::known(1), Clock::from_counter(now)).await;
    match opened {
        Err(TaskError::Common(Error::Unavailable(why))) => {
            assert!(
                why.contains("step 2"),
                "the step that differs is named: {why}"
            );
        }
        Err(other) => panic!("an unexpected error: {other:?}"),
        Ok(_) => panic!("the file of run 2a must not open until the steps are frozen"),
    }
}
