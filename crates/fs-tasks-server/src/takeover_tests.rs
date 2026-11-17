//! Tests of the rule for a server that does not answer, on made-up facts: one test per rule.
//!
//! Pattern: Table tests — each rule of `decide` with the one fact that breaks it; every rule has a mutant (the rule
//! removed) that turns its test red.

use crate::takeover::{Decision, Image, Own, Peer, Why, decide};

const ME: u32 = 1000;

fn image(ino: u64, path: &str) -> Image {
    Image {
        dev: 7,
        ino,
        path: path.into(),
    }
}

fn own() -> Own {
    Own {
        pid: 500,
        uid: ME,
        exe: image(10, "/usr/bin/fs-tasks"),
    }
}

fn peer() -> Peer {
    Peer {
        pid: 900,
        uid: ME,
        start: Some(4242),
        exe: Some(image(10, "/usr/bin/fs-tasks")),
    }
}

#[test]
fn the_same_program_of_the_same_user_seen_before_is_taken_over() {
    assert_eq!(decide(Some(4242), &peer(), &own()), Decision::TakeOver);
}

#[test]
fn never_the_system_or_this_process() {
    for pid in [1, 0, -1, 500] {
        let p = Peer { pid, ..peer() };
        assert_eq!(
            decide(Some(4242), &p, &own()),
            Decision::Refuse(Why::Itself),
            "{pid}"
        );
    }
}

#[test]
fn never_a_process_of_another_user() {
    let p = Peer { uid: 0, ..peer() };
    assert_eq!(
        decide(Some(4242), &p, &own()),
        Decision::Refuse(Why::ForeignUser)
    );
    let unclear = Peer {
        uid: u32::MAX,
        ..peer()
    };
    assert_eq!(
        decide(Some(4242), &unclear, &own()),
        Decision::Refuse(Why::ForeignUser)
    );
}

#[test]
fn a_reused_process_number_is_not_the_process_that_was_seen() {
    // Same number, started later: another process now. Also a different start time after the dialog.
    let p = Peer {
        start: Some(9999),
        ..peer()
    };
    assert_eq!(
        decide(Some(4242), &p, &own()),
        Decision::Refuse(Why::PidReused)
    );
}

#[test]
fn what_cannot_be_read_cannot_be_verified() {
    let no_start = Peer {
        start: None,
        ..peer()
    };
    assert_eq!(
        decide(Some(4242), &no_start, &own()),
        Decision::Refuse(Why::Unreadable)
    );
    assert_eq!(
        decide(None, &peer(), &own()),
        Decision::Refuse(Why::Unreadable)
    );
    let no_exe = Peer {
        exe: None,
        ..peer()
    };
    assert_eq!(
        decide(Some(4242), &no_exe, &own()),
        Decision::Refuse(Why::Unreadable)
    );
}

#[test]
fn another_program_file_is_refused_even_with_the_same_name() {
    // The same file name in another place is another program: never ended, only reported.
    let p = Peer {
        exe: Some(image(11, "/home/x/bin/fs-tasks")),
        ..peer()
    };
    assert_eq!(
        decide(Some(4242), &p, &own()),
        Decision::Refuse(Why::OtherProgram)
    );
}

#[test]
fn a_link_or_hard_link_to_this_program_and_a_replaced_file_are_this_program() {
    // The same file reached by another path (a link was followed, or a hard link): same device and file number.
    let linked = Peer {
        exe: Some(image(10, "/home/x/link-to-tasks")),
        ..peer()
    };
    assert_eq!(decide(Some(4242), &linked, &own()), Decision::TakeOver);
    // The file at the path was replaced by an update: the old process runs the old file under the same real path.
    let updated = Peer {
        exe: Some(image(99, "/usr/bin/fs-tasks")),
        ..peer()
    };
    assert_eq!(decide(Some(4242), &updated, &own()), Decision::TakeOver);
}
