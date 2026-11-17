//! A process that holds the socket of a tasks server and never answers, made of the test program itself.
//!
//! Pattern: Test double — the test binary starts itself again (`--exact stub_main`) with an environment variable that
//! turns that one test into the double: it binds the socket and holds its locks like a server, and never accepts. Its
//! `/proc/<pid>/exe` is the test binary, so it is "this program" for the rule; a copy of the binary elsewhere is "another
//! program". Spawns and copies go through one mutex with `sync_all`, so no process is started while a file is still open
//! for writing (ETXTBSY).

#![allow(dead_code)] // each test file uses a different subset

use std::fs::File;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

/// The socket the double binds (the variable that turns the test into the double).
pub const SOCKET: &str = "FS_TASKS_STUB_SOCKET";
/// Set: the double ignores SIGTERM.
pub const IGNORE_TERM: &str = "FS_TASKS_STUB_IGNORE_TERM";
/// Set: the double also holds the start lock (`tasks.lock`).
pub const START_LOCK: &str = "FS_TASKS_STUB_START_LOCK";
/// A folder: the process is a real server (`serve_command`) with this data folder.
pub const SERVE: &str = "FS_TASKS_STUB_SERVE";

static SPAWN: Mutex<()> = Mutex::new(());

/// The body of the test `stub_main` of each test file: nothing unless the process was started as the double.
pub fn stub_main() {
    let Some(socket) = std::env::var_os(SOCKET).map(PathBuf::from) else {
        return;
    };
    if let Some(data) = std::env::var_os(SERVE) {
        let code = fs_tasks_server::serve_command([
            "--socket".into(),
            socket.into_os_string(),
            "--data".into(),
            data,
        ]);
        std::process::exit(code);
    }
    let mut name = socket.clone().into_os_string();
    name.push(".lock");
    let lock = File::create(PathBuf::from(name)).unwrap();
    lock.lock().unwrap();
    let start = File::create(socket.with_extension("lock")).unwrap();
    if std::env::var_os(START_LOCK).is_some() {
        start.lock().unwrap();
    }
    let _listener = UnixListener::bind(&socket).unwrap();
    if std::env::var_os(IGNORE_TERM).is_some() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let _term =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();
            loop {
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        });
    }
    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}

/// Start `exe` as the double (an `exe` that is a copy of the test binary, or a link to it); wait until it holds the socket.
pub fn start_stub(exe: &Path, socket: &Path, extra: &[(&str, &str)]) -> Child {
    let child = {
        let _one = SPAWN.lock().unwrap_or_else(PoisonError::into_inner);
        let mut command = Command::new(exe);
        command
            .args(["--exact", "stub_main", "--nocapture", "--test-threads=1"])
            .env(SOCKET, socket)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        for (name, value) in extra {
            command.env(name, value);
        }
        command.spawn().unwrap()
    };
    let until = Instant::now() + Duration::from_secs(10);
    while !socket.exists() {
        assert!(
            Instant::now() < until,
            "the double did not bind {}",
            socket.display()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    child
}

/// A copy of this test binary in `dir`, with the same file name: another program file with the same content and name.
pub fn copy_of_this_program(dir: &Path) -> PathBuf {
    let _one = SPAWN.lock().unwrap_or_else(PoisonError::into_inner);
    let exe = std::env::current_exe().unwrap();
    let copy = dir.join(exe.file_name().unwrap());
    std::fs::copy(&exe, &copy).unwrap();
    std::fs::set_permissions(&copy, PermissionsExt::from_mode(0o755)).unwrap();
    File::open(&copy).unwrap().sync_all().unwrap();
    copy
}

/// The way the double (and a real server) is started by `ensure_server_with`: this test binary again, as a real server.
pub fn serve_starter(
    pids: std::sync::Arc<Mutex<Vec<u32>>>,
) -> impl Fn(&Path, Option<&Path>) -> Result<Child, String> + Send + 'static {
    move |socket, data| {
        let _one = SPAWN.lock().unwrap_or_else(PoisonError::into_inner);
        let child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "stub_main", "--nocapture", "--test-threads=1"])
            .env(SOCKET, socket)
            .env(SERVE, data.unwrap())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        pids.lock().unwrap().push(child.id());
        Ok(child)
    }
}

/// Wait until `child` has ended, at most `limit`; `true` if it has.
pub fn ended(child: &mut Child, limit: Duration) -> bool {
    let until = Instant::now() + limit;
    loop {
        if child.try_wait().unwrap().is_some() {
            return true;
        }
        if Instant::now() >= until {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// End a process of this test (its own child) by its number, whatever it is doing.
pub fn kill_own_child(pid: u32) {
    if let Some(pid) = rustix::process::Pid::from_raw(i32::try_from(pid).unwrap()) {
        let _ = rustix::process::kill_process(pid, rustix::process::Signal::KILL);
    }
}
