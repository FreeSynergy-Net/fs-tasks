//! Tests of the failure texts: code and parameters for the machine, the sentence in German and English for the person.
//!
//! Pattern: Table tests — one silent-server failure with all its parameters, in both languages, askable and not.

use crate::datadir_text::Lang;
use crate::startfail::{StartError, Stuck};

fn stuck(can_ask: bool) -> StartError {
    StartError::Unresponsive(Stuck {
        pid: Some(4711),
        start: Some(99),
        since: Some("2026-11-17T08:00:00Z".into()),
        exe: Some("/usr/bin/fs-tasks".into()),
        can_ask,
        why: if can_ask {
            "other-program"
        } else {
            "foreign-user"
        },
    })
}

#[test]
fn the_report_is_code_and_parameters_then_the_sentence_in_the_language_of_the_person() {
    let error = stuck(true);
    assert_eq!(error.code(), "server-unresponsive");
    assert_eq!(
        error.params(),
        "pid=4711 since=2026-11-17T08:00:00Z exe=/usr/bin/fs-tasks"
    );
    let en = error.report(Lang::En);
    assert!(
        en.starts_with(
            "server-unresponsive pid=4711 since=2026-11-17T08:00:00Z exe=/usr/bin/fs-tasks\n"
        ),
        "{en}"
    );
    assert!(
        en.contains("already running, but it does not answer") && en.ends_with("End it?"),
        "{en}"
    );
    let de = error.report(Lang::De);
    assert!(
        de.contains("läuft schon, reagiert aber nicht") && de.ends_with("Beenden?"),
        "{de}"
    );
    assert!(
        de.contains("PID 4711") && de.contains("/usr/bin/fs-tasks"),
        "{de}"
    );
    assert_eq!(error.exit_code(), 3);
}

#[test]
fn a_process_that_may_not_be_ended_is_not_offered_for_ending() {
    let text = stuck(false).report(Lang::En);
    assert!(
        text.contains("not ended from here (reason: foreign-user)"),
        "{text}"
    );
    assert!(!text.contains("End it?"), "{text}");
    assert!(stuck(false).stuck().is_some_and(|s| !s.can_ask));
}

#[test]
fn the_other_failures_have_their_codes_and_exit_codes() {
    let locked = StartError::StartLocked {
        path: "/run/tasks.lock".into(),
    };
    assert_eq!((locked.code(), locked.exit_code()), ("start-locked", 3));
    assert!(locked.report(Lang::De).contains("path=/run/tasks.lock"));
    let other = StartError::OtherProgram {
        program: "fs-other".into(),
    };
    assert_eq!((other.code(), other.exit_code()), ("other-program", 1));
    assert_eq!(StartError::Failed("x".into()).to_string(), "x");
}
