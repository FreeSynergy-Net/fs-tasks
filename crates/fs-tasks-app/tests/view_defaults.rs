//! A task typed in a view takes what the view gives it, so the view shows it at once; and what cannot be shown or
//! made is told, never swallowed.
//!
//! Pattern: Black-box tests through the screens (`PlanSurface`) over the real core and adapter; the failing store is
//! a Decorator over the real core whose writes are refused (the tasks_v1 `FailingTasks` double has the locked
//! `Tasks` trait, not the `TaskItems` the window works on).

mod common;

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use common::World;
use fs_objects::ObjectId;
use fs_tasks_app::plan_view::{
    Cache, NewTaskDefaults, Notice, PlanSurface, View, board_columns, defaults_for, fits, landing,
    notice_text, sections,
};
use fs_tasks_app::{Day, TaskAdapter};
use fs_tasks_core::plan::Readiness;
use fs_tasks_core::{
    Capabilities, Place, Result, TaskEntry, TaskItems, TaskPage, TaskQuery, TaskRepo,
};
use fs_tasks_server::Lang;
use serde_json::json;

fn today() -> Day {
    Day::parse("2026-11-12").unwrap()
}

fn labelled(w: &World, n: u8, title: &str, label: &str) {
    w.add_json(json!({
        "@type": "Task", "version": "2.0",
        "uid": format!("urn:uuid:019a0000-0000-7000-8000-0000000000{n:02}"),
        "title": title, "keywords": {label: true}
    }));
}

/// A store with a task in project A and one in project B, and the screens over it.
fn screens() -> (World, Arc<Cache>, PlanSurface) {
    let w = World::new();
    labelled(&w, 1, "Old A", "A");
    labelled(&w, 2, "Old B", "B");
    let items: Arc<dyn TaskItems> = w.repo.clone();
    let cache = Cache::start(items, w.runtime.handle().clone(), today());
    cache.wait(Duration::from_secs(120)).expect("a picture");
    let surface = PlanSurface::new(w.adapter.clone(), cache.clone(), false);
    (w, cache, surface)
}

fn on_board(cache: &Cache, label: Option<&str>) -> Vec<String> {
    let snap = cache.current().unwrap();
    board_columns(&snap, label, None)
        .into_iter()
        .flat_map(|c| c.cards.into_iter().map(|k| k.title))
        .collect()
}

fn in_list(cache: &Cache, place: &str) -> Vec<String> {
    let snap = cache.current().unwrap();
    sections(&snap, place, &[])
        .into_iter()
        .flat_map(|s| s.lines.into_iter().map(|l| l.title))
        .collect()
}

#[test]
fn a_task_typed_on_the_board_with_a_project_chosen_has_that_project_and_is_shown() {
    let (_w, cache, s) = screens();
    s.show_label(Some("A"));
    s.submit("needs-action", "New in A");
    assert!(on_board(&cache, Some("A")).contains(&"New in A".to_string()));
    assert!(on_board(&cache, Some("B")).iter().all(|t| t != "New in A"));
    assert_eq!(s.notice(), None, "it is in the view: nothing to say");
}

#[test]
fn a_task_typed_in_a_board_column_lands_in_that_column() {
    let (_w, cache, s) = screens();
    s.show_label(Some("A"));
    s.submit("in-process", "Started in A");
    let snap = cache.current().unwrap();
    let cols = board_columns(&snap, Some("A"), None);
    let col = cols.iter().find(|c| c.key == "in-process").unwrap();
    assert!(col.cards.iter().any(|c| c.title == "Started in A"));
}

#[test]
fn a_task_typed_in_a_project_list_has_the_project() {
    let (_w, cache, s) = screens();
    s.set_list(true);
    s.show_list("label:A");
    s.submit("", "List A");
    assert!(in_list(&cache, "label:A").contains(&"List A".to_string()));
    assert_eq!(s.notice(), None);
}

#[test]
fn a_task_typed_in_today_and_upcoming_is_due_then_and_shown() {
    let (_w, cache, s) = screens();
    s.set_list(true);
    s.show_list("today");
    s.submit("", "For today");
    assert!(in_list(&cache, "today").contains(&"For today".to_string()));
    s.show_list("upcoming");
    s.submit("", "For tomorrow");
    assert!(in_list(&cache, "upcoming").contains(&"For tomorrow".to_string()));
    assert_eq!(s.notice(), None);
}

#[test]
fn a_task_typed_in_inbox_and_anytime_is_shown() {
    let (_w, cache, s) = screens();
    s.set_list(true);
    s.show_list("inbox");
    s.submit("", "Into the inbox");
    assert!(in_list(&cache, "inbox").contains(&"Into the inbox".to_string()));
    s.show_list("anytime");
    s.submit("", "Some time");
    assert!(in_list(&cache, "anytime").contains(&"Some time".to_string()));
    assert_eq!(s.notice(), None);
}

#[test]
fn a_task_typed_in_the_logbook_or_someday_says_where_it_is() {
    let (_w, cache, s) = screens();
    s.set_list(true);
    for (place, home) in [("logbook", "inbox"), ("someday", "inbox")] {
        s.show_list(place);
        s.submit("", "Wrong place");
        assert!(!in_list(&cache, place).contains(&"Wrong place".to_string()));
        assert!(in_list(&cache, home).contains(&"Wrong place".to_string()));
        assert_eq!(
            s.notice_data(),
            Some(Notice::OutsideView {
                title: "Wrong place".into(),
                place: home.into()
            }),
            "{place}"
        );
        assert!(!s.notice().unwrap().is_empty(), "the sentence is shown");
    }
}

#[test]
fn the_notice_is_a_code_with_parameters_in_both_languages() {
    let n = Notice::OutsideView {
        title: "T".into(),
        place: "label:Haus".into(),
    };
    assert_eq!(n.code(), "task.created-outside-view");
    assert_eq!(
        notice_text(&n, Lang::En),
        "Created \"T\" in Haus, not in the current filter."
    );
    assert_eq!(
        notice_text(&n, Lang::De),
        "\u{201e}T\u{201c} angelegt in Haus, nicht im aktuellen Filter."
    );
}

#[test]
fn the_defaults_are_a_pure_rule_per_view() {
    let t = today();
    let board = View::Board {
        label: Some("A"),
        column: "in-process",
    };
    assert_eq!(
        defaults_for(&board, t),
        NewTaskDefaults {
            labels: vec!["A".into()],
            progress: Some("in-process".into()),
            due: None
        }
    );
    assert_eq!(
        defaults_for(&View::List { place: "upcoming" }, t).due,
        Some(t.plus(1))
    );
    let blank = NewTaskDefaults::default();
    assert_eq!(landing(&blank, t), "inbox");
    assert!(!fits(&View::List { place: "logbook" }, &blank, t));
    assert!(!fits(&View::List { place: "someday" }, &blank, t));
    assert!(fits(&View::List { place: "inbox" }, &blank, t));
}

/// The core for reading; every write is refused (the provider says `task.create` is not possible).
struct ReadOnly(Arc<TaskRepo>);

#[async_trait]
impl TaskItems for ReadOnly {
    fn capabilities(&self) -> Capabilities {
        self.0.capabilities()
    }
    async fn get(&self, id: ObjectId) -> Result<Option<TaskEntry>> {
        TaskItems::get(self.0.as_ref(), id).await
    }
    async fn get_by_uid(&self, uid: &str) -> Result<Option<TaskEntry>> {
        self.0.get_by_uid(uid).await
    }
    async fn list(&self) -> Result<Vec<TaskEntry>> {
        TaskItems::list(self.0.as_ref()).await
    }
    async fn query(&self, q: &TaskQuery) -> Result<Vec<TaskEntry>> {
        self.0.query(q).await
    }
    async fn page(&self, q: &TaskQuery, after: Option<&Place>, size: usize) -> Result<TaskPage> {
        self.0.page(q, after, size).await
    }
    async fn readiness_many(&self, ids: &[ObjectId]) -> Result<Vec<(ObjectId, Readiness)>> {
        self.0.readiness_many(ids).await
    }
}

#[test]
fn a_refused_create_is_told_and_makes_nothing() {
    let w = World::new();
    labelled(&w, 1, "Old A", "A");
    let items: Arc<dyn TaskItems> = Arc::new(ReadOnly(w.repo.clone()));
    let adapter = Arc::new(TaskAdapter::new(items.clone(), w.runtime.handle().clone()));
    let cache = Cache::start(items, w.runtime.handle().clone(), today());
    cache.wait(Duration::from_secs(120)).expect("a picture");
    let s = PlanSurface::new(adapter, cache.clone(), false);
    s.show_label(Some("A"));
    s.submit("needs-action", "Cannot be made");
    match s.notice_data() {
        Some(Notice::CreateFailed { title, detail, .. }) => {
            assert_eq!(title, "Cannot be made", "the typed text is in the message");
            assert!(detail.contains("task.create"), "{detail}");
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
    assert!(s.notice().unwrap().contains("Cannot be made"));
    assert_eq!(on_board(&cache, None), ["Old A"], "nothing was made");
    // the list screen tells it the same way
    s.set_list(true);
    s.submit("", "Cannot be made either");
    assert_eq!(s.notice_data().unwrap().code(), "task.create-failed");
}
