//! What only `LocalTasks` can show: the rows (typed columns, the deadline, the O18 detach), the file (it survives a
//! reopen), and the honest refusal of what this run does not store.
//!
//! Pattern: Rows-level tests (the table and the file are looked at directly, what the trait cannot show).

#[allow(dead_code)]
mod common;

use std::sync::Arc;
use std::sync::atomic::AtomicI64;

use common::{LocalFx, count, draft};
use fs_objects::v5::codec::get_int;
use fs_objects::v5::spec::Logical;
use fs_tasks_local::tables::ALL;
use fs_tasks_testdata::tasks_v1::{
    Error, Restorable, SeriesScope, TaskError, TaskPatch, Tasks, Trash,
};
use fs_tasks_testdata::{START_MS, person};

const DAY: i64 = 86_400_000;

#[test]
fn no_column_is_a_blob_or_a_json_text() {
    for t in &ALL {
        for c in t.cols {
            assert!(
                c.logical != Logical::Blob && !c.name.contains("json"),
                "{}.{} is an untyped column",
                t.name,
                c.name
            );
        }
    }
}

#[tokio::test]
async fn the_file_keeps_lists_tasks_and_labels_across_a_reopen() {
    let path = format!("{}/reopen.db", env!("CARGO_TARGET_TMPDIR"));
    let _ = std::fs::remove_file(&path);
    let now = Arc::new(AtomicI64::new(START_MS));
    let (list, task) = {
        let fx = LocalFx::open(&path, &now).await;
        let list = fx
            .tasks
            .create_list("Home", Some("#abcdef"), None)
            .await
            .unwrap();
        let rest = TaskPatch {
            labels: Some(vec!["b".into(), "a".into()]),
            priority: Some(3),
            ..TaskPatch::default()
        };
        let task = fx
            .tasks
            .create(draft(list.id, "Water the plants", rest))
            .await
            .unwrap();
        (list, task)
    };
    let fx = LocalFx::open(&path, &now).await;
    assert_eq!(fx.tasks.get(task.id).await.unwrap(), Some(task.clone()));
    assert_eq!(task.labels, ["a", "b"]);
    let lists = fx.tasks.lists(Trash::Exclude).await.unwrap();
    assert_eq!(lists.len(), 1);
    assert_eq!((lists[0].id, lists[0].name.as_str()), (list.id, "Home"));
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn the_deadline_is_the_time_of_the_delete_plus_the_trash_period() {
    let fx = LocalFx::memory().await;
    fx.tasks.set_trash_days(7).await.unwrap();
    let list = fx.tasks.create_list("L", None, None).await.unwrap();
    let t = fx
        .tasks
        .create(draft(list.id, "T", TaskPatch::default()))
        .await
        .unwrap();
    let d = fx
        .tasks
        .delete(t.id, t.revision, SeriesScope::Whole)
        .await
        .unwrap();
    let Some(Restorable::Until(until)) = d.restorable else {
        panic!("a deadline is `Until`: {:?}", d.restorable);
    };
    let gap = until.millis() - (START_MS + 7 * DAY);
    assert!(
        (0..1000).contains(&gap),
        "7 days after the delete, got {gap} ms off"
    );
    let mut q = fs_tasks_testdata::tasks_v1::TaskQuery {
        lists: vec![list.id],
        text: None,
        progress: Vec::new(),
        due: None,
        undated: None,
        assignee: None,
        labels: Vec::new(),
        parent: None,
        ready: None,
        order: fs_tasks_testdata::tasks_v1::Order::Rank,
        view_zone: fs_tasks_testdata::tasks_v1::Zone::iana("UTC").unwrap(),
        trash: Trash::Only,
    };
    let seen = fx.tasks.list(&q, None, 10).await.unwrap();
    assert_eq!(seen.items[0].trash, Some(Restorable::Until(until)));
    q.trash = Trash::Exclude;
    let alive = fx.tasks.list(&q, None, 10).await.unwrap();
    assert_eq!(alive.items.len(), 0);
}

#[tokio::test]
async fn after_the_deadline_the_rows_are_gone_with_their_labels() {
    let fx = LocalFx::memory().await;
    let list = fx.tasks.create_list("L", None, None).await.unwrap();
    let rest = TaskPatch {
        labels: Some(vec!["x".into()]),
        ..TaskPatch::default()
    };
    let t = fx.tasks.create(draft(list.id, "T", rest)).await.unwrap();
    fx.tasks
        .delete(t.id, t.revision, SeriesScope::Whole)
        .await
        .unwrap();
    assert_eq!(
        (
            count(&fx, "tasks_task").await,
            count(&fx, "tasks_label").await
        ),
        (1, 1)
    );
    fx.tasks.set_trash_days(1).await.unwrap();
    common::advance(&fx, 31 * DAY);
    assert!(fx.tasks.get(t.id).await.unwrap().is_none());
    assert_eq!(
        (
            count(&fx, "tasks_task").await,
            count(&fx, "tasks_label").await
        ),
        (0, 0)
    );
    assert_eq!(count(&fx, "objects").await, 1, "only the list is left");
}

#[tokio::test]
async fn a_subtask_deleted_earlier_is_hung_off_before_its_parent_goes() {
    let fx = LocalFx::memory().await;
    let list = fx.tasks.create_list("L", None, None).await.unwrap();
    let p = fx
        .tasks
        .create(draft(list.id, "parent", TaskPatch::default()))
        .await
        .unwrap();
    let under = TaskPatch {
        parent: Some(Some(p.id)),
        ..TaskPatch::default()
    };
    let with = fx
        .tasks
        .create(draft(list.id, "with", under.clone()))
        .await
        .unwrap();
    let early = fx
        .tasks
        .create(draft(list.id, "early", under))
        .await
        .unwrap();
    let early_gone = fx
        .tasks
        .delete(early.id, early.revision, SeriesScope::Whole)
        .await
        .unwrap();
    let p = fx.tasks.get(p.id).await.unwrap().unwrap();
    let gone = fx
        .tasks
        .delete(p.id, p.revision, SeriesScope::Whole)
        .await
        .unwrap();
    assert_eq!(gone.with_subtasks, [with.id]);
    fx.tasks.purge(p.id).await.unwrap();
    assert!(fx.tasks.restore(with.id, SeriesScope::Whole).await.is_err());
    let back = fx
        .tasks
        .restore(early_gone.id, SeriesScope::Whole)
        .await
        .unwrap();
    assert_eq!(
        back.parent, None,
        "the early subtask lost its parent and kept its own deadline"
    );
    let c = fx.tasks.store().read().await;
    let rows = c
        .query(
            "SELECT count(*) FROM tasks_task WHERE parent IS NOT NULL",
            Vec::new(),
        )
        .await
        .unwrap();
    assert_eq!(get_int(&rows[0], 0).unwrap(), 0);
}

#[tokio::test]
async fn what_this_run_does_not_store_is_refused_by_name_never_dropped() {
    let fx = LocalFx::memory().await;
    let list = fx.tasks.create_list("L", None, None).await.unwrap();
    let mut d = draft(list.id, "T", TaskPatch::default());
    d.assignees = vec![person(5)];
    let r = fx.tasks.create(d).await;
    assert!(
        matches!(r, Err(TaskError::Common(Error::Unsupported { .. }))),
        "{r:?}"
    );
    let rest = TaskPatch {
        recurrence: Some(vec![fs_tasks_testdata::tasks_v1::Recurrence::Original(
            "FREQ=DAILY".into(),
        )]),
        ..TaskPatch::default()
    };
    assert!(matches!(
        fx.tasks.create(draft(list.id, "T", rest)).await,
        Err(TaskError::Common(Error::Unsupported { .. }))
    ));
    assert_eq!(count(&fx, "tasks_task").await, 0, "nothing was written");
    let group = fs_tasks_testdata::tasks_v1::PersonRef {
        art: fs_tasks_testdata::tasks_v1::PersonArt::Group,
        ..person(9)
    };
    let owned = fx.tasks.create_list("G", None, Some(&group)).await;
    assert!(
        matches!(owned, Err(TaskError::Common(Error::Forbidden { .. }))),
        "{owned:?}"
    );
}
