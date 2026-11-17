//! The real program against a server that holds the socket and never answers: it ends with code and message, never silent.
//!
//! Pattern: Test double — the test binary starts itself again as a process that binds the socket and never answers (it is
//! another program than `fs-tasks`, so it is asked about, not ended). Up to 0.0.9 `fs-tasks` then neither opened a window
//! nor said anything. The command line ends with exit code 4 (3 is the claim conflict there), the window command with 3;
//! both name the code `server-unresponsive` with the process, on stderr. The window is not shown: the test has no display.
#![cfg(unix)]

use std::os::unix::net::UnixListener;
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const SOCKET: &str = "FS_TASKS_HANG_STUB";

static SPAWN: Mutex<()> = Mutex::new(());

#[test]
fn stub_main() {
    let Some(socket) = std::env::var_os(SOCKET) else {
        return;
    };
    let mut lock = socket.clone();
    lock.push(".lock");
    let lock = std::fs::File::create(lock).unwrap();
    lock.lock().unwrap();
    let _listener = UnixListener::bind(&socket).unwrap();
    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}

fn silent_server(socket: &Path) -> Child {
    let child = {
        let _one = SPAWN
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "stub_main", "--nocapture", "--test-threads=1"])
            .env(SOCKET, socket)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap()
    };
    let until = Instant::now() + Duration::from_secs(10);
    while !socket.exists() {
        assert!(Instant::now() < until, "the double did not bind");
        std::thread::sleep(Duration::from_millis(20));
    }
    child
}

/// Run `fs-tasks args` (no display, German off), at most 40 s: a hang fails the test.
fn run(args: &[&str], runtime_dir: &Path) -> (Output, Duration) {
    let began = Instant::now();
    let mut child = Command::new(env!("CARGO_BIN_EXE_fs-tasks"))
        .args(args)
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .env("XDG_RUNTIME_DIR", runtime_dir)
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    while child.try_wait().unwrap().is_none() {
        if began.elapsed() > Duration::from_secs(40) {
            child.kill().unwrap();
            panic!("fs-tasks {args:?} did not end: it hangs, silently");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    (child.wait_with_output().unwrap(), began.elapsed())
}

fn setup() -> (tempfile::TempDir, std::path::PathBuf, Child) {
    let dir = tempfile::tempdir().unwrap();
    let run_dir = dir.path().join("run");
    std::fs::create_dir(&run_dir).unwrap();
    std::fs::set_permissions(
        &run_dir,
        std::os::unix::fs::PermissionsExt::from_mode(0o700),
    )
    .unwrap();
    let socket = run_dir.join("tasks.sock");
    let silent = silent_server(&socket);
    (dir, socket, silent)
}

#[test]
fn the_command_line_says_that_the_server_does_not_answer_and_ends_with_code_4() {
    let (dir, socket, mut silent) = setup();
    let (out, took) = run(&["--socket", socket.to_str().unwrap(), "list"], dir.path());
    let err = String::from_utf8_lossy(&out.stderr);
    let alive = silent.try_wait().unwrap().is_none();
    silent.kill().unwrap();
    silent.wait().unwrap();
    assert_eq!(out.status.code(), Some(4), "{err}");
    assert!(
        err.contains(&format!("server-unresponsive pid={}", silent.id())),
        "{err}"
    );
    assert!(
        err.contains("does not answer")
            && err.contains("not ended from here")
            && !err.contains("End it?"),
        "{err}"
    );
    assert!(
        alive,
        "a process of another program was ended without a yes"
    );
    assert!(took < Duration::from_secs(20), "{took:?}");
}

#[cfg(feature = "gui")]
#[test]
fn the_window_command_ends_with_code_3_and_the_same_message_even_when_no_window_can_open() {
    let (dir, socket, mut silent) = setup();
    let (out, _) = run(&["--connect", socket.to_str().unwrap()], dir.path());
    let err = String::from_utf8_lossy(&out.stderr);
    silent.kill().unwrap();
    silent.wait().unwrap();
    assert_eq!(out.status.code(), Some(3), "{err}");
    assert!(
        err.contains(&format!("server-unresponsive pid={}", silent.id())),
        "{err}"
    );
}
