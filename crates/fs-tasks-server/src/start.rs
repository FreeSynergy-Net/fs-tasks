//! Making sure a server answers, starting the local one if it is not there, never waiting for ever.
//!
//! Pattern: Double-checked lock with a time limit on every wait — shake hands with the socket (`handshake`); if nobody
//! answers, take the lock file next to the socket (try-lock, waiting at most `lock_wait`), shake hands again (another
//! client may have started the server meanwhile), and only then start `fs-tasks serve` and wait for it. A server that
//! holds the socket but does not answer is looked at (`ending`): this program of this user is ended and replaced, in
//! any other case the failure says who holds the socket (code `server-unresponsive`). Many clients at once (the command
//! line, the window, agents) start one server, not many. The server is its own process (own process group): it
//! outlives the client that started it.

use std::fs::{OpenOptions, TryLockError};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::ending::{Freed, describe, free_socket};
use crate::handshake::{Probe, probe};
use crate::startfail::StartError;

/// The time limits of the start. The defaults are what the program uses; tests make them shorter.
#[derive(Clone, Copy, Debug)]
pub struct Times {
    /// One step of the handshake (connect, ask): about two seconds.
    pub call: Duration,
    /// How long to wait for the start lock of another client.
    pub lock_wait: Duration,
    /// A server younger than this may still be opening its store (the start of a client waits five seconds for it, so this is
    /// longer): it gets this long to answer before it counts as silent.
    pub young: Duration,
    /// After SIGTERM: how long until SIGKILL.
    pub term: Duration,
    /// After SIGKILL: how long until it is called not ending.
    pub kill: Duration,
    /// How long a server that was started gets to answer.
    pub start: Duration,
}

impl Default for Times {
    fn default() -> Self {
        Self {
            call: Duration::from_secs(2),
            lock_wait: Duration::from_secs(2),
            young: Duration::from_secs(6),
            term: Duration::from_secs(2),
            kill: Duration::from_secs(2),
            start: Duration::from_secs(5),
        }
    }
}

/// How the server is started: given the socket and the data folder, a child process.
pub type Spawn<'a> = &'a dyn Fn(&Path, Option<&Path>) -> Result<Child, String>;

fn spawn(socket: &Path, data: Option<&Path>) -> Result<Child, String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("cannot find myself to start the server: {e}"))?;
    let mut command = Command::new(exe);
    command.arg("serve").arg("--socket").arg(socket);
    if let Some(data) = data {
        command.arg("--data").arg(data);
    }
    // What the server says when it cannot start goes to `<socket>.log` (0600, one file, overwritten at each start), so the
    // client can tell the person the cause.
    let log = crate::serverlog::open_fresh(&crate::serverlog::path_for(socket))
        .map_or_else(|_| Stdio::null(), Stdio::from);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(log);
    // Its own process group: a Ctrl-C meant for the client does not stop the server.
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    command
        .spawn()
        .map_err(|e| format!("cannot start the server: {e}"))
}

/// Return once a server answers at `socket`; start one with `data` as its data folder if none does.
///
/// # Errors
/// The server could not be started, did not come up, or a server holds the socket and does not answer.
pub async fn ensure_server(socket: &Path, data: Option<&Path>) -> Result<(), StartError> {
    ensure_server_with(socket, data, &Times::default(), &spawn).await
}

/// [`ensure_server`] with other time limits and another way to start the server (for tests and other starters).
///
/// # Errors
/// As for [`ensure_server`].
pub async fn ensure_server_with(
    socket: &Path,
    data: Option<&Path>,
    times: &Times,
    spawn: Spawn<'_>,
) -> Result<(), StartError> {
    let mut silent = match probe(socket, times.call).await {
        Probe::Alive => return Ok(()),
        Probe::Absent => false,
        Probe::Silent => true,
        Probe::Other(program) => return Err(StartError::OtherProgram { program }),
        Probe::Failed(text) => return Err(StartError::Failed(text)),
    };
    if let Some(dir) = socket.parent() {
        crate::datadir::create_private_dir(dir).map_err(StartError::Failed)?;
    }
    let path = socket.with_extension("lock");
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .map_err(|e| StartError::Failed(format!("{}: {e}", path.display())))?;
    // Held until this function returns (the lock goes with the file handle).
    if lock_within(&lock, times.lock_wait).await? {
        // Under the lock nobody else starts one: shake hands again. A server that was silent a moment ago gets a short look.
        let second = if silent {
            times.call.min(Duration::from_millis(500))
        } else {
            times.call
        };
        silent = match probe(socket, second).await {
            Probe::Alive => return Ok(()),
            Probe::Absent => false,
            Probe::Silent => true,
            Probe::Other(program) => return Err(StartError::OtherProgram { program }),
            Probe::Failed(text) => return Err(StartError::Failed(text)),
        };
    } else {
        // Somebody else holds the start lock: another start under way, or a server that holds it and is stuck. One more
        // look: if it is silent, it is looked at like any silent server; if nothing is there, a message, never a wait.
        match probe(socket, times.call).await {
            Probe::Alive => return Ok(()),
            Probe::Silent => silent = true,
            _ => return Err(StartError::StartLocked { path }),
        }
    }
    if silent && free_socket(socket, times).await? == Freed::Ready {
        return Ok(());
    }
    start_and_wait(socket, data, times, spawn).await
}

/// Take the lock, trying every 50 ms for at most `wait`.
async fn lock_within(lock: &std::fs::File, wait: Duration) -> Result<bool, StartError> {
    let until = Instant::now() + wait;
    loop {
        match lock.try_lock() {
            Ok(()) => return Ok(true),
            Err(TryLockError::WouldBlock) => {
                if Instant::now() >= until {
                    return Ok(false);
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Err(TryLockError::Error(e)) => return Err(StartError::Failed(e.to_string())),
        }
    }
}

/// Start the server and wait for its answer, at most `times.start`.
async fn start_and_wait(
    socket: &Path,
    data: Option<&Path>,
    times: &Times,
    spawn: Spawn<'_>,
) -> Result<(), StartError> {
    let mut child = spawn(socket, data).map_err(StartError::Failed)?;
    let until = Instant::now() + times.start;
    let mut last = Probe::Absent;
    while Instant::now() < until {
        tokio::time::sleep(Duration::from_millis(50)).await;
        last = probe(socket, times.call.min(Duration::from_millis(500))).await;
        if last == Probe::Alive {
            return Ok(());
        }
        if let Ok(Some(status)) = child.try_wait() {
            return Err(ended(
                socket,
                &format!("the server ended at once ({status})"),
            ));
        }
    }
    if last == Probe::Silent {
        // It holds the socket and does not answer: say who it is, so the person can end it.
        return Err(describe(socket, times).await);
    }
    Err(ended(
        socket,
        &format!("the server did not come up at {}", socket.display()),
    ))
}

/// The start error: what happened, then the last lines the server wrote (the cause), and where the whole log is.
fn ended(socket: &Path, what: &str) -> StartError {
    let log = crate::serverlog::path_for(socket);
    StartError::Failed(match crate::serverlog::last_lines(&log) {
        Some(lines) => format!("{what}; its log {}:\n{lines}", log.display()),
        None => format!("{what}; it wrote nothing to {}", log.display()),
    })
}
