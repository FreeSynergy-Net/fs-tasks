//! A server that holds the socket and never answers: the start is never silent, and it takes the server over when it may.
//!
//! Pattern: Test double — a second process (the test binary again, see `stub`) binds the socket, holds the locks and
//! never answers; then the start of a second client runs against it. Until 0.0.10 that start waited for ever, with no
//! window and no message. Every rule of the take-over has a test here (or in `takeover_tests`) and a mutant that turns it red.
#![cfg(unix)]

mod stub;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use fs_tasks_core::TaskItems;
use fs_tasks_server::{RemoteItems, StartError, Times, end_confirmed, ensure_server_with};
use stub::{IGNORE_TERM, START_LOCK};

#[test]
fn stub_main() {
    stub::stub_main();
}

fn times() -> Times {
    Times {
        call: Duration::from_millis(300),
        lock_wait: Duration::from_millis(300),
        young: Duration::ZERO,
        term: Duration::from_millis(400),
        kill: Duration::from_secs(3),
        start: Duration::from_secs(20),
    }
}

struct Place {
    dir: tempfile::TempDir,
    socket: PathBuf,
    data: PathBuf,
}

fn place() -> Place {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("run").join("tasks.sock");
    std::fs::create_dir_all(socket.parent().unwrap()).unwrap();
    std::fs::set_permissions(
        socket.parent().unwrap(),
        std::os::unix::fs::PermissionsExt::from_mode(0o700),
    )
    .unwrap();
    let data = dir.path().join("data");
    Place { dir, socket, data }
}

/// `ensure_server_with` on a thread of its own, answered within 40 s: a start that hangs fails the test, not the run.
fn ensure(p: &Place, times: Times, pids: &Arc<Mutex<Vec<u32>>>) -> Result<(), StartError> {
    let (socket, data) = (p.socket.clone(), p.data.clone());
    let starter = stub::serve_starter(Arc::clone(pids));
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let local = tokio::task::LocalSet::new();
        let _ = tx.send(local.block_on(
            &runtime,
            ensure_server_with(&socket, Some(&data), &times, &starter),
        ));
    });
    rx.recv_timeout(Duration::from_secs(40))
        .expect("the start did not end in time: it hangs")
}

fn stop_what_we_started(p: &Place, pids: &Arc<Mutex<Vec<u32>>>) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = tokio::task::LocalSet::new();
    let _ = local.block_on(&runtime, fs_interfaces::rpc::stop_server(&p.socket));
    std::thread::sleep(Duration::from_millis(300));
    for pid in pids.lock().unwrap().iter() {
        stub::kill_own_child(*pid);
    }
}

#[test]
fn a_silent_server_of_this_program_that_also_holds_the_start_lock_is_ended_and_replaced() {
    let (p, pids) = (place(), Arc::new(Mutex::new(Vec::new())));
    let mut silent = stub::start_stub(
        &std::env::current_exe().unwrap(),
        &p.socket,
        &[(START_LOCK, "1")],
    );
    let began = Instant::now();
    let result = ensure(&p, times(), &pids);
    assert_eq!(result, Ok(()), "{result:?}");
    assert!(
        stub::ended(&mut silent, Duration::from_secs(3)),
        "the silent server was not ended"
    );
    assert!(began.elapsed() < Duration::from_secs(30));
    // The new one is a real server: the window's connection works.
    let remote = RemoteItems::connect(p.socket.clone(), Some(p.data.clone())).unwrap();
    assert!(remote.capabilities().is_supported("task.create"));
    drop(remote);
    stop_what_we_started(&p, &pids);
    assert_eq!(pids.lock().unwrap().len(), 1, "one new server, not two");
}

#[test]
fn a_silent_server_that_ignores_sigterm_is_killed() {
    let (p, pids) = (place(), Arc::new(Mutex::new(Vec::new())));
    let mut silent = stub::start_stub(
        &std::env::current_exe().unwrap(),
        &p.socket,
        &[(IGNORE_TERM, "1")],
    );
    assert_eq!(ensure(&p, times(), &pids), Ok(()));
    assert!(
        stub::ended(&mut silent, Duration::from_secs(3)),
        "SIGTERM was ignored and nothing killed it"
    );
    stop_what_we_started(&p, &pids);
}

#[test]
fn a_link_to_this_program_is_this_program() {
    let (p, pids) = (place(), Arc::new(Mutex::new(Vec::new())));
    let link = p.dir.path().join("link-to-tasks");
    std::os::unix::fs::symlink(std::env::current_exe().unwrap(), &link).unwrap();
    let mut silent = stub::start_stub(&link, &p.socket, &[]);
    assert_eq!(ensure(&p, times(), &pids), Ok(()));
    assert!(stub::ended(&mut silent, Duration::from_secs(3)));
    stop_what_we_started(&p, &pids);
}

#[test]
fn another_program_file_is_never_ended_and_has_no_yes() {
    let (p, pids) = (place(), Arc::new(Mutex::new(Vec::new())));
    let other = stub::copy_of_this_program(&p.dir.path().join("run"));
    let mut silent = stub::start_stub(&other, &p.socket, &[]);
    let Err(StartError::Unresponsive(stuck)) = ensure(&p, times(), &pids) else {
        panic!("the start must say that the server does not answer");
    };
    assert_eq!(stuck.pid, Some(i32::try_from(silent.id()).unwrap()));
    assert!(!stuck.can_ask, "no yes for another program: {stuck:?}");
    assert_eq!(stuck.why, "other-program");
    assert!(
        stuck
            .exe
            .as_deref()
            .is_some_and(|e| e.ends_with(other.file_name().unwrap().to_str().unwrap())),
        "{stuck:?}"
    );
    assert!(
        !stub::ended(&mut silent, Duration::from_millis(600)),
        "another program was ended"
    );
    assert!(pids.lock().unwrap().is_empty(), "no new server was started");
    // Not even a forged yes (the same process as shown, marked askable) ends it.
    let forged = fs_tasks_server::Stuck {
        can_ask: true,
        ..stuck.clone()
    };
    assert!(end_confirmed(&p.socket, &forged).is_err());
    assert!(!stub::ended(&mut silent, Duration::from_millis(300)));
    // The message names the process and offers nothing to end.
    let text = StartError::Unresponsive(stuck).report(fs_tasks_server::Lang::En);
    assert!(text.contains("pid=") && !text.contains("End it?"), "{text}");
}

#[test]
fn a_young_server_gets_its_time_before_it_counts_as_silent() {
    let (p, pids) = (place(), Arc::new(Mutex::new(Vec::new())));
    let mut silent = stub::start_stub(&std::env::current_exe().unwrap(), &p.socket, &[]);
    let young = Times {
        young: Duration::from_secs(3),
        ..times()
    };
    let began = Instant::now();
    assert_eq!(ensure(&p, young, &pids), Ok(()));
    assert!(
        began.elapsed() >= Duration::from_secs(2),
        "it was ended after {:?}",
        began.elapsed()
    );
    assert!(stub::ended(&mut silent, Duration::from_secs(3)));
    stop_what_we_started(&p, &pids);
}

#[test]
fn a_start_lock_held_by_somebody_ends_with_a_message_not_with_a_wait() {
    let (p, pids) = (place(), Arc::new(Mutex::new(Vec::new())));
    let held = std::fs::File::create(p.socket.with_extension("lock")).unwrap();
    held.lock().unwrap();
    let began = Instant::now();
    let result = ensure(&p, times(), &pids);
    assert!(
        matches!(result, Err(StartError::StartLocked { .. })),
        "{result:?}"
    );
    assert!(began.elapsed() < Duration::from_secs(5));
    assert!(
        pids.lock().unwrap().is_empty(),
        "no server was started under somebody else's lock"
    );
    assert_eq!(result.unwrap_err().exit_code(), 3);
}

#[test]
fn a_server_of_another_program_is_named_and_left_alone() {
    let (p, pids) = (place(), Arc::new(Mutex::new(Vec::new())));
    let socket = p.socket.clone();
    let server = std::thread::spawn(move || {
        fs_interfaces::rpc::run_until_signal_as(
            &socket,
            fs_interfaces::rpc::ServerIdentity::new("fs-other", "1"),
            || async {
                let store: Arc<dyn fs_objects::Store> = Arc::new(
                    fs_objects::TursoStore::open_in_memory()
                        .await
                        .map_err(std::io::Error::other)?,
                );
                let repo = Arc::new(fs_tasks_core::TaskRepo::new(
                    store,
                    "https://local.invalid/users/owner",
                ));
                Ok(fs_tasks_server::TasksServer::new(repo.clone(), repo)
                    .into_client()
                    .client)
            },
        )
    });
    let until = Instant::now() + Duration::from_secs(10);
    while !Path::new(&p.socket).exists() {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(20));
    }
    let result = ensure(&p, times(), &pids);
    assert_eq!(
        result,
        Err(StartError::OtherProgram {
            program: "fs-other".into()
        })
    );
    assert!(pids.lock().unwrap().is_empty());
    stop_what_we_started(&p, &pids);
    server.join().unwrap().unwrap();
}
