//! Alerts and links in `LocalTasks`: typed rows (no JSON), the checksum of a file as 64 hex digits, a size that does not fit
//! a signed number is `Invalid`, and the rows go with their task at the final delete.
//!
//! Pattern: Behaviour tests (one scenario each, through the trait; the rows are looked at with SQL).

#[allow(dead_code)]
mod common;

use common::{LocalFx, count, draft};
use fs_objects::v5::codec::get_text;
use fs_tasks_testdata::tasks_v1::{
    Alert, AlertAction, Error, FileRef, Link, MediaType, RelativeTo, SeriesScope, TaskError,
    TaskPatch, Tasks, Trigger, Utc,
};
use fs_types::{BasicValue, Reference};

fn file_link(size: Option<u64>) -> Link {
    Link {
        key: "f".into(),
        href: None,
        file: Some(FileRef {
            iri: Reference::parse(
                "urn:sha256:00000000000000000000000000000000000000000000000000000000000000ab",
            )
            .unwrap(),
            sha256: [0xab; 32],
            media_type: MediaType("image/png".into()),
        }),
        content_type: None,
        size,
        rel: None,
        title: None,
    }
}

#[tokio::test]
async fn alerts_and_links_are_typed_rows_and_go_with_their_task() {
    let fx = LocalFx::memory().await;
    let list = fx.tasks.create_list("L", None, None).await.unwrap().id;
    let rest = TaskPatch {
        alerts: Some(vec![Alert {
            key: "a".into(),
            trigger: Trigger::Offset {
                seconds: -60,
                relative_to: RelativeTo::End,
            },
            action: AlertAction::Display,
            acknowledged: Some(Utc::from_millis(5)),
        }]),
        links: Some(vec![file_link(Some(7))]),
        ..TaskPatch::default()
    };
    let t = fx.tasks.create(draft(list, "T", rest)).await.unwrap();
    assert_eq!(
        (
            count(&fx, "tasks_alert").await,
            count(&fx, "tasks_link").await
        ),
        (1, 1)
    );
    {
        let c = fx.tasks.store().read().await;
        let rows = c
            .query(
                "SELECT file_sha256, trigger_kind, relative_to FROM tasks_link l, tasks_alert a",
                Vec::new(),
            )
            .await
            .unwrap();
        assert_eq!(
            get_text(&rows[0], 0).unwrap(),
            "ab".repeat(32),
            "the checksum is 64 hex digits"
        );
        assert_eq!(get_text(&rows[0], 1).unwrap(), "offset");
        assert_eq!(get_text(&rows[0], 2).unwrap(), "end");
    }
    fx.tasks
        .delete(t.id, t.revision, SeriesScope::Whole)
        .await
        .unwrap();
    fx.tasks.purge(t.id).await.unwrap();
    assert_eq!(
        (
            count(&fx, "tasks_alert").await,
            count(&fx, "tasks_link").await
        ),
        (0, 0)
    );
}

#[tokio::test]
async fn a_link_size_that_does_not_fit_a_signed_number_is_invalid_and_nothing_is_stored() {
    let fx = LocalFx::memory().await;
    let list = fx.tasks.create_list("L", None, None).await.unwrap().id;
    let rest = TaskPatch {
        links: Some(vec![file_link(Some(u64::MAX))]),
        ..TaskPatch::default()
    };
    let r = fx.tasks.create(draft(list, "T", rest)).await;
    assert!(
        matches!(r, Err(TaskError::Common(Error::Invalid(_)))),
        "{r:?}"
    );
    assert_eq!(count(&fx, "tasks_link").await, 0);
    assert_eq!(
        count(&fx, "tasks_task").await,
        0,
        "the failed create left no task"
    );
}
