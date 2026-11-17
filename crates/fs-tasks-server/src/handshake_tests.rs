//! Tests of the handshake: a server that answers is there, one that holds the socket and does not is silent, none is absent.
//!
//! Pattern: Table tests on real sockets in a temporary folder — the double is a listener in this process that never accepts.

use std::os::unix::net::UnixListener;
use std::time::{Duration, Instant};

use crate::handshake::{Probe, probe};

fn local() -> (tokio::runtime::Runtime, tokio::task::LocalSet) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    (runtime, tokio::task::LocalSet::new())
}

#[test]
fn nobody_there_is_absent() {
    let dir = tempfile::tempdir().unwrap();
    let (runtime, local) = local();
    let found = local.block_on(
        &runtime,
        probe(&dir.path().join("tasks.sock"), Duration::from_millis(300)),
    );
    assert_eq!(found, Probe::Absent);
}

#[test]
fn a_listener_that_never_answers_is_silent_within_the_time_not_for_ever() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tasks.sock");
    let _listener = UnixListener::bind(&path).unwrap();
    let (runtime, local) = local();
    let began = Instant::now();
    let found = local.block_on(&runtime, probe(&path, Duration::from_millis(300)));
    assert_eq!(found, Probe::Silent);
    assert!(
        began.elapsed() < Duration::from_secs(2),
        "{:?}",
        began.elapsed()
    );
}
