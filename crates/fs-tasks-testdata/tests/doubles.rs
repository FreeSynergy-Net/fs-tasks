//! `FailingTasks`, `SlowTasks`, `HeldTasks`, `FakePeople` and the additions of the sample (PLAN section 10).

use fs_tasks_testdata::tasks_v1::{
    Date, DateField, DueRange, Error, Order, Progress, Task, TaskDraft, TaskError, TaskPatch,
    TaskQuery, Tasks, Trash, Zone,
};
use fs_tasks_testdata::{
    FailingTasks, FakePeople, FakeTasks, HeldTasks, KNOWN, Op, SlowTasks, forbidden, sample,
    unavailable,
};
use fs_types::BasicValue;
use std::sync::Arc;

fn query(lists: Vec<fs_tasks_testdata::tasks_v1::ObjectId>) -> TaskQuery {
    TaskQuery {
        lists,
        text: None,
        progress: Vec::new(),
        due: None,
        undated: None,
        assignee: None,
        labels: Vec::new(),
        parent: None,
        ready: None,
        order: Order::Rank,
        view_zone: Zone::iana("Europe/Berlin").unwrap(),
        trash: Trash::Exclude,
    }
}

#[tokio::test]
async fn a_failing_provider_answers_the_chosen_error_and_heals() {
    let s = sample();
    let broken = s.lists[1].id;
    let f = FailingTasks::new(Arc::new(s.fake()))
        .failing(Op::Lists, unavailable("service ended"))
        .failing(Op::Create, forbidden("task.create"))
        .failing_list(broken, unavailable("list unreadable"));
    assert!(matches!(
        f.lists(Trash::Exclude).await,
        Err(TaskError::Common(Error::Unavailable(_)))
    ));
    assert!(matches!(
        f.create(TaskDraft {
            list: s.lists[0].id,
            uid: None,
            title: "x".to_owned(),
            progress: Progress::NeedsAction,
            assignees: Vec::new(),
            checklist: Vec::new(),
            rest: TaskPatch::default(),
        })
        .await,
        Err(TaskError::Common(Error::Forbidden { .. }))
    ));
    assert!(f.list(&query(vec![broken]), None, 10).await.is_err());
    assert!(f.list(&query(vec![s.lists[0].id]), None, 10).await.is_ok());
    assert!(f.asker().await.is_ok());
    f.heal();
    assert!(f.lists(Trash::Exclude).await.is_ok());
    assert!(f.list(&query(vec![broken]), None, 10).await.is_ok());
    let all = FailingTasks::new(Arc::new(FakeTasks::new())).failing_all(unavailable("gone"));
    assert!(all.get(s.tasks[0].1.id).await.is_err());
    assert_eq!(all.capabilities().0.len(), 22);
}

#[tokio::test]
async fn a_slow_provider_moves_the_fake_clock_and_never_sleeps() {
    let fake = Arc::new(sample().fake());
    let start = fake.now();
    let slow = SlowTasks::of_fake(fake.clone(), 1000).taking(Op::List, 3000);
    let t = std::time::Instant::now();
    slow.lists(Trash::Exclude).await.unwrap();
    slow.list(&query(Vec::new()), None, 5).await.unwrap();
    assert_eq!(slow.waited_ms(), 4000);
    assert_eq!(fake.now().millis() - start.millis(), 4000);
    assert!(t.elapsed().as_secs() < 2, "no real sleep");
}

#[tokio::test]
async fn a_broken_list_breaks_the_query_for_all_lists_too() {
    // Z4: a query without lists means "all lists", so the broken one is among them.
    let s = sample();
    let broken = s.lists[1].id;
    let f =
        FailingTasks::new(Arc::new(s.fake())).failing_list(broken, unavailable("list unreadable"));
    assert!(f.list(&query(Vec::new()), None, 10).await.is_err());
    assert!(f.list(&query(vec![s.lists[0].id]), None, 10).await.is_ok());
}

#[tokio::test]
async fn a_held_call_stays_open_until_release_so_a_frontend_can_show_loading() {
    let held = Arc::new(HeldTasks::new(Arc::new(sample().fake())));
    held.hold();
    let call = {
        let h = held.clone();
        tokio::spawn(async move { h.lists(Trash::Exclude).await })
    };
    for _ in 0..20 {
        tokio::task::yield_now().await;
    }
    assert_eq!(held.waiting(), 1, "the call waits");
    assert!(!call.is_finished(), "and is not answered");
    held.release();
    let lists = call.await.unwrap().unwrap();
    assert_eq!(lists.len(), 6);
    assert_eq!(held.waiting(), 0);
    // Open gate: calls pass at once.
    assert!(held.asker().await.is_ok());
}

#[tokio::test]
async fn a_dropped_held_call_is_not_counted_any_more() {
    let held = Arc::new(HeldTasks::new(Arc::new(sample().fake())));
    held.hold();
    let call = {
        let h = held.clone();
        tokio::spawn(async move { h.asker().await })
    };
    for _ in 0..20 {
        tokio::task::yield_now().await;
    }
    assert_eq!(held.waiting(), 1);
    call.abort();
    let _ = call.await;
    assert_eq!(held.waiting(), 0);
}

#[tokio::test]
async fn the_people_are_the_ones_of_the_sample_with_their_names_and_kinds() {
    use fs_people_directory::PersonDirectory;
    let people = FakePeople::new();
    for k in KNOWN {
        let r = people.resolve(&k.person()).await.unwrap();
        assert!(
            format!("{r:?}").contains(k.display),
            "{} is not shown as {}",
            k.number,
            k.display
        );
    }
    let me = people.asker().await.unwrap();
    assert_eq!(me, Some(KNOWN[0].person()));
}

#[tokio::test]
async fn the_people_come_from_the_directory_fake() {
    use fs_people_directory::PersonDirectory;
    let people = FakePeople::new();
    let me = people.asker().await.unwrap();
    assert!(me.is_some());
    let p = FakePeople::person(1).expect("sample person 1");
    assert!(people.resolve(&p).await.is_ok());
}

fn task<'a>(s: &'a fs_tasks_testdata::Sample, slug: &str) -> &'a Task {
    s.try_get(slug).expect("a task of the sample")
}

#[tokio::test]
async fn the_additions_cover_new_york_priority_9_assumed_zone_alert_link_and_the_long_column() {
    let s = sample();
    assert!(s.try_get("gibt-es-nicht").is_none());
    let ny = task(&s, "ny-abstimmung");
    assert!(matches!(
        &ny.due,
        Some(fs_tasks_testdata::tasks_v1::Due::Timed { zone, zone_assumed: false, .. }) if zone.id() == "America/New_York"
    ));
    let pass = task(&s, "reisepass");
    assert_eq!(pass.priority, 9);
    assert_eq!(pass.alerts.len(), 1);
    assert_eq!(pass.links.len(), 1);
    assert!(matches!(
        &task(&s, "import-ohne-zone").due,
        Some(fs_tasks_testdata::tasks_v1::Due::Timed {
            zone_assumed: true,
            ..
        })
    ));
    let column = s
        .tasks
        .iter()
        .filter(|(_, t)| t.list == s.lists[4].id)
        .count();
    assert_eq!(column, 40);
    // 15:00 in New York is 21:00 in Berlin: still the 16th in Berlin, so "today" there.
    let fake = s.fake();
    let day = Date::parse("2026-11-16").unwrap();
    let mut q = query(Vec::new());
    q.due = Some(DueRange {
        field: DateField::Due,
        from: Some(day),
        to: Some(day),
    });
    let today = fake.list(&q, None, 200).await.unwrap().items;
    assert!(today.iter().any(|x| x.id == ny.id));
}
