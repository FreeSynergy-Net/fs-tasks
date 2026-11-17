//! The window that says the server does not answer: the sentence is in it, and the two answers work.
//!
//! Pattern: Shell state tests — the window is built (no screen needed) and its status line, menus and handler are read;
//! the picture is drawn where there is a graphics adapter and skipped with a message where there is none.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use fs_components_app::{AppError, Status};
use fs_components_render::RenderError;
use fs_tasks_app::problem_window::{End, build};
use fs_tasks_server::{Lang, StartError, Stuck};

static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    ONE_AT_A_TIME.lock().unwrap_or_else(PoisonError::into_inner)
}

fn silent(can_ask: bool) -> StartError {
    StartError::Unresponsive(Stuck {
        pid: Some(4711),
        start: Some(99),
        since: Some("2026-11-17T08:00:00Z".into()),
        exe: Some("/usr/bin/fs-tasks".into()),
        can_ask,
        why: "other-program",
    })
}

fn counting() -> (End, Arc<AtomicUsize>) {
    let ended = Arc::new(AtomicUsize::new(0));
    let again = Arc::clone(&ended);
    let end: End = Arc::new(move || {
        again.fetch_add(1, Ordering::SeqCst);
        Ok(())
    });
    (end, ended)
}

#[test]
fn the_window_names_the_process_asks_to_end_it_and_says_so_in_german_too() {
    let (end, _) = counting();
    let app = build(&silent(true), Lang::En, end, Arc::default()).unwrap();
    let status = app.with_shell(|s| s.status().cloned());
    let Some(Status::Problem(text)) = status else {
        panic!("the status line must say the problem");
    };
    assert!(
        text.contains("PID 4711")
            && text.contains("/usr/bin/fs-tasks")
            && text.ends_with("End it?"),
        "{text}"
    );
    let entries = app.with_shell(|s| {
        s.menus()
            .iter()
            .flat_map(|m| {
                m.entries
                    .iter()
                    .map(|e| (e.id.clone(), e.label.clone(), e.disabled))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    });
    assert_eq!(entries.len(), 2, "{entries:?}");
    assert_eq!(entries[0].1, "Yes: end it and start again");
    assert!(!entries[0].2, "yes is possible for a process of the person");
    let (end, _) = counting();
    let german = build(&silent(true), Lang::De, end, Arc::default()).unwrap();
    let Some(Status::Problem(text)) = german.with_shell(|s| s.status().cloned()) else {
        panic!("no status");
    };
    assert!(
        text.contains("reagiert aber nicht") && text.ends_with("Beenden?"),
        "{text}"
    );
}

#[test]
fn yes_ends_the_server_and_asks_for_another_try_no_ends_nothing() {
    let (end, ended) = counting();
    let retry = Arc::new(AtomicBool::new(false));
    let app = build(&silent(true), Lang::En, end, Arc::clone(&retry)).unwrap();
    assert!(app.choose("server-close"));
    assert_eq!(ended.load(Ordering::SeqCst), 0, "no ends nothing");
    assert!(!retry.load(Ordering::SeqCst));
    assert!(app.choose("server-end"));
    assert_eq!(ended.load(Ordering::SeqCst), 1);
    assert!(
        retry.load(Ordering::SeqCst),
        "the program must try again after the server was ended"
    );
}

#[test]
fn a_failed_ending_is_shown_not_swallowed_and_a_process_that_may_not_be_asked_about_has_no_yes() {
    let failing: End = Arc::new(|| Err(StartError::Failed("process 4711 does not end".into())));
    let retry = Arc::new(AtomicBool::new(false));
    let app = build(&silent(true), Lang::En, failing, Arc::clone(&retry)).unwrap();
    assert!(app.choose("server-end"));
    assert!(!retry.load(Ordering::SeqCst));
    let Some(Status::Problem(text)) = app.with_shell(|s| s.status().cloned()) else {
        panic!("no status");
    };
    assert!(text.contains("does not end"), "{text}");
    let (end, ended) = counting();
    let refused = build(&silent(false), Lang::En, end, Arc::default()).unwrap();
    assert!(
        !refused.choose("server-end"),
        "there is no yes for another program"
    );
    let entries = refused.with_shell(|s| {
        s.menus()
            .iter()
            .map(|m| m.entries.len())
            .collect::<Vec<_>>()
    });
    assert_eq!(entries, vec![1], "only Close is offered");
    let Some(Status::Problem(text)) = refused.with_shell(|s| s.status().cloned()) else {
        panic!("no status");
    };
    assert!(
        text.contains("PID 4711")
            && text.contains("/usr/bin/fs-tasks")
            && !text.contains("End it?"),
        "{text}"
    );
    assert_eq!(ended.load(Ordering::SeqCst), 0);
}

#[test]
fn the_window_is_drawn_with_its_sentence() {
    let _one = serial();
    let (end, _) = counting();
    let app = build(&silent(true), Lang::En, end, Arc::default()).unwrap();
    let path = std::env::temp_dir().join(format!("fs-tasks-problem-{}.png", std::process::id()));
    match app.picture(900, 400, &path) {
        Err(AppError::Picture(RenderError::NoGraphics(why))) => eprintln!("skipped: {why}"),
        other => {
            other.unwrap();
            let bytes = std::fs::read(PathBuf::from(&path)).unwrap();
            std::fs::remove_file(&path).unwrap();
            assert!(bytes.len() > 1000, "an empty picture");
        }
    }
}
