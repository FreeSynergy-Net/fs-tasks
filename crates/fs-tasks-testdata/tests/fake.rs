//! Behaviour of the fake that the conformance rules cannot reach: the clock, claims of others, repeated writes,
//! the boundary and the capabilities.

use fs_tasks_testdata::tasks_v1::{
    Capabilities, Children, Error, Progress, Restorable, SeriesScope, TaskDraft, TaskError,
    TaskExchange, TaskPatch, Tasks, Trash, Utc,
};
use fs_tasks_testdata::{DECISIONS, FakeExchange, FakeTasks, MEDIA_TYPES, person, sample};

fn draft(list: fs_tasks_testdata::tasks_v1::ObjectId, title: &str) -> TaskDraft {
    TaskDraft {
        list,
        uid: None,
        title: title.to_owned(),
        progress: Progress::NeedsAction,
        assignees: Vec::new(),
        checklist: Vec::new(),
        rest: TaskPatch::default(),
    }
}

fn on(caps: &Capabilities, name: &str) -> bool {
    caps.0.iter().any(|(c, on)| c == name && *on)
}

#[tokio::test]
async fn a_claim_of_another_person_holds_until_it_runs_out() {
    let s = sample();
    let fake = s.fake();
    let kasse = s.get("kasse");
    let held = fake
        .claim(
            kasse.id,
            kasse.revision,
            Utc::from_millis(fake.now().millis() + 1000),
        )
        .await;
    assert!(matches!(held, Err(TaskError::Claimed(c)) if c.by == person(2)));
    let released = fake.release(kasse.id, kasse.revision).await;
    assert!(matches!(released, Err(TaskError::Claimed(_))));
    fake.set_now(Utc::from_millis(fake.now().millis() + 4 * 86_400_000));
    let free = fake.release(kasse.id, kasse.revision).await.unwrap();
    assert!(free.claim.is_none());
}

#[tokio::test]
async fn the_trash_is_gone_after_thirty_days_with_the_whole_list() {
    let fake = FakeTasks::new();
    let list = fake.create_list("Weg", None, None).await.unwrap();
    let t = fake.create(draft(list.id, "x")).await.unwrap();
    let d = fake.delete_list(list.id, list.revision).await.unwrap();
    fake.set_now(Utc::from_millis(
        match d.restorable {
            Some(Restorable::Until(t)) => t.millis(),
            other => panic!("a deadline, not {other:?}"),
        } + 1,
    ));
    assert!(fake.restore_list(list.id).await.is_err());
    assert_eq!(fake.lists(Trash::Include).await.unwrap(), Vec::new());
    assert!(!fake.knows(t.id));
}

#[tokio::test]
async fn an_update_that_changes_nothing_writes_nothing() {
    let fake = FakeTasks::new();
    let list = fake.create_list("L", None, None).await.unwrap();
    let t = fake.create(draft(list.id, "x")).await.unwrap();
    let before = fake.changes(None, 1).await.unwrap().next.unwrap();
    let same = fake
        .update(
            t.id,
            t.revision,
            TaskPatch {
                title: Some("x".into()),
                ..Default::default()
            },
            SeriesScope::Whole,
        )
        .await
        .unwrap();
    assert_eq!(same.revision, t.revision);
    assert_eq!(
        fake.changes(Some(&before), 10).await.unwrap().items,
        Vec::new()
    );
}

#[tokio::test]
async fn the_trash_of_a_parent_takes_the_children_and_a_failed_split_leaves_no_trace() {
    let s = sample();
    let fake = s.fake();
    let p = s.get("einpacken");
    let d = fake
        .delete(p.id, p.revision, SeriesScope::Whole)
        .await
        .unwrap();
    assert_eq!(d.with_subtasks.len(), 3);
    let kid = s.get("buecher");
    assert!(fake.get(kid.id).await.unwrap().is_none());
    // A series split followed by a refused write is rolled back (no half state).
    let series = s.get("pflanzen");
    let rid = fs_tasks_testdata::occurrence("2026-11-20T00:00:00").unwrap();
    let stale = fake
        .update(
            series.id,
            fs_types_revision(series.revision.0 + 9),
            TaskPatch::default(),
            SeriesScope::ThisAndFollowing { recurrence_id: rid },
        )
        .await;
    assert!(matches!(
        stale,
        Err(TaskError::Common(Error::Changed { .. }))
    ));
    let head = fake.get_by_uid(&series.uid).await.unwrap().unwrap();
    assert_eq!(head.related, Vec::new());
}

fn fs_types_revision(n: u64) -> fs_tasks_testdata::tasks_v1::Revision {
    fs_tasks_testdata::tasks_v1::Revision(n)
}

#[tokio::test]
async fn open_subtasks_refuse_and_same_changes_them_in_one_call() {
    let s = sample();
    let fake = s.fake();
    let p = s.get("einkauf");
    let refused = fake
        .progress(
            p.id,
            p.revision,
            Progress::Completed,
            Children::Refuse,
            SeriesScope::Whole,
        )
        .await;
    assert!(matches!(refused, Err(TaskError::OpenSubtasks(ids)) if ids.len() == 2));
    let done = fake
        .progress(
            p.id,
            p.revision,
            Progress::Completed,
            Children::Same,
            SeriesScope::Whole,
        )
        .await
        .unwrap();
    assert_eq!(done.progress, Progress::Completed);
    assert_eq!(
        fake.get(s.get("milch").id).await.unwrap().unwrap().progress,
        Progress::Completed
    );
}

#[test]
fn the_capabilities_say_what_the_fake_can() {
    let caps = FakeTasks::new().capabilities();
    assert_eq!(caps.0.len(), 22);
    assert!(caps.0.iter().all(|(_, on)| *on));
    let caps = FakeTasks::new().without_evaluation().capabilities();
    assert!(!on(&caps, "dependency.evaluate") && on(&caps, "relation.create"));
    assert_eq!(DECISIONS.len(), 14);
}

#[test]
fn the_exchange_is_honest_about_what_it_does_not_do() {
    let formats = FakeExchange.formats();
    assert_eq!(formats.len(), MEDIA_TYPES.len());
    assert!(
        formats
            .iter()
            .all(|f| f.import == fs_tasks_testdata::tasks_v1::Level::No)
    );
}
