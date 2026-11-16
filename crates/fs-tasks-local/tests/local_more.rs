//! More of what only `LocalTasks` can show: the database counts the labels, who sees which list (a colleague, an AI, a
//! list made read only), and the checklist rows that go with their task.
//!
//! Pattern: Behaviour tests (one scenario each, through the trait only).

#[allow(dead_code)]
mod common;

use common::{LocalFx, count, draft};
use fs_tasks_testdata::tasks_v1::{
    Error, ListAccess, SeriesScope, TaskError, TaskPatch, Tasks, Trash,
};

#[tokio::test]
async fn labels_are_counted_by_the_database() {
    let fx = LocalFx::memory().await;
    let list = fx.tasks.create_list("L", None, None).await.unwrap();
    let with = |l: &[&str]| TaskPatch {
        labels: Some(l.iter().map(|x| (*x).to_owned()).collect()),
        ..TaskPatch::default()
    };
    for (t, l) in [
        ("1", &["zz-b", "zz-a"][..]),
        ("2", &["zz-b", "yy"]),
        ("3", &["zz-c"]),
    ] {
        fx.tasks.create(draft(list.id, t, with(l))).await.unwrap();
    }
    let all = fx.tasks.labels(&[list.id], Some("zz-"), None, 10).await;
    println!("{all:?}");
    let names: Vec<(String, u32)> = all
        .unwrap()
        .items
        .into_iter()
        .map(|l| (l.label, l.count))
        .collect();
    assert_eq!(
        names,
        [("zz-a".into(), 1), ("zz-b".into(), 2), ("zz-c".into(), 1)]
    );
}

#[tokio::test]
async fn somebody_else_sees_nothing_of_my_lists_and_an_ai_owns_none() {
    use fs_tasks_testdata::tasks_v1::ListAccess;
    let fx = LocalFx::memory().await;
    let list = fx.tasks.create_list("Mine", None, None).await.unwrap();
    let t = fx
        .tasks
        .create(draft(list.id, "T", TaskPatch::default()))
        .await
        .unwrap();
    let (colleague, agent) = (common::known(2), common::known(8));
    fx.tasks.act_as(Some(colleague));
    assert_eq!(fx.tasks.lists(Trash::Include).await.unwrap().len(), 0);
    assert!(fx.tasks.get(t.id).await.unwrap().is_none());
    let r = fx
        .tasks
        .update(t.id, t.revision, TaskPatch::default(), SeriesScope::Whole)
        .await;
    assert!(
        matches!(r, Err(TaskError::Common(Error::NotFound(_)))),
        "{r:?}"
    );
    fx.tasks.act_as(Some(agent.clone()));
    let own = fx.tasks.create_list("AI", None, None).await;
    assert!(
        matches!(own, Err(TaskError::Common(Error::Forbidden { .. }))),
        "{own:?}"
    );
    fx.tasks.act_as(None);
    fx.tasks.make_read_only(&agent, list.id).await.unwrap();
    fx.tasks.act_as(Some(agent));
    let seen = fx.tasks.lists(Trash::Exclude).await.unwrap();
    assert_eq!((seen.len(), seen[0].access), (1, ListAccess::Read));
    let made = fx
        .tasks
        .create(draft(list.id, "no", TaskPatch::default()))
        .await;
    assert!(
        matches!(made, Err(TaskError::Common(Error::Forbidden { .. }))),
        "{made:?}"
    );
}

#[tokio::test]
async fn the_checklist_lives_in_its_table_and_goes_with_its_task() {
    use fs_tasks_testdata::tasks_v1::ChecklistChange;
    let fx = LocalFx::memory().await;
    let list = fx.tasks.create_list("L", None, None).await.unwrap();
    let mut d = draft(list.id, "T", TaskPatch::default());
    d.checklist = vec!["one".into(), "two".into()];
    let t = fx.tasks.create(d).await.unwrap();
    let t = fx
        .tasks
        .checklist(
            t.id,
            t.revision,
            ChecklistChange::Check {
                key: t.checklist[0].key.clone(),
                done: true,
            },
        )
        .await
        .unwrap();
    assert_eq!(count(&fx, "tasks_checklist").await, 2);
    assert_eq!(fx.tasks.get(t.id).await.unwrap(), Some(t.clone()));
    assert!(t.checklist[0].done.is_some() && t.checklist[1].done.is_none());
    let d = fx
        .tasks
        .delete(t.id, t.revision, SeriesScope::Whole)
        .await
        .unwrap();
    fx.tasks.purge(d.id).await.unwrap();
    assert_eq!(
        count(&fx, "tasks_checklist").await,
        0,
        "the items went with the task"
    );
}

/// A stored level that nobody knows is an error, never `Manage` (a guess would open a list that was meant to be closed).
#[test]
fn a_level_word_nobody_knows_is_an_error_not_manage() {
    use fs_tasks_local::access::level_of;
    assert_eq!(level_of("read").unwrap(), ListAccess::Read);
    assert_eq!(level_of("write").unwrap(), ListAccess::Write);
    assert_eq!(level_of("manage").unwrap(), ListAccess::Manage);
    assert!(level_of("admin").is_err());
    assert!(level_of("").is_err());
}
