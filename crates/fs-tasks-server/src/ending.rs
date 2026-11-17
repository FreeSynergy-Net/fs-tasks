//! Ending a server of this program that holds the socket and does not answer, safely (Unix).
//!
//! Pattern: Command guarded by `takeover::decide` — the process behind the socket is found through the socket itself
//! (`SO_PEERCRED` of a connection: the process that listens, no scan of all processes), looked at twice (before and
//! after a `pidfd` is taken, so a reused process number cannot be hit), judged by `decide`, then ended with
//! SIGTERM and, after a short time, SIGKILL through that `pidfd`. Only the socket file that belonged to the ended
//! process is removed. Every step is written to `<socket>.takeover.log` (0600) and stderr.

use std::io::Write;
use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt};
use std::path::Path;
use std::time::{Duration, Instant};

use rustix::process::{Pid, PidfdFlags, Signal, pidfd_open, pidfd_send_signal};

use crate::handshake::{Probe, probe};
use crate::procfs::{self, PROC};
use crate::start::Times;
use crate::startfail::{StartError, Stuck};
use crate::takeover::{Decision, Own, Peer, decide};

/// The process that was seen: its number and its start time, which a confirmation is checked against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Target {
    /// The process number.
    pub pid: i32,
    /// The start time in clock ticks since the boot.
    pub start: u64,
}

/// What `free_socket` found.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Freed {
    /// The server answers now (it was only busy starting).
    Ready,
    /// The socket is free: nobody holds it any more.
    Free,
}

enum Listener {
    Gone,
    Unknown,
    Process(i32, u32),
}

/// Who listens at `socket`: asked through a connection (the kernel tells the process that called `listen`).
async fn listener(socket: &Path, call: Duration) -> Listener {
    match tokio::time::timeout(call, tokio::net::UnixStream::connect(socket)).await {
        Err(_) => Listener::Unknown,
        Ok(Err(_)) => Listener::Gone,
        Ok(Ok(stream)) => match stream.peer_cred() {
            Ok(cred) => cred
                .pid()
                .map_or(Listener::Unknown, |pid| Listener::Process(pid, cred.uid())),
            Err(_) => Listener::Unknown,
        },
    }
}

fn failed(text: impl Into<String>) -> StartError {
    StartError::Failed(text.into())
}

/// The failure for a process that is not ended (or not without asking).
fn stuck_of(peer: &Peer, decision: Decision) -> StartError {
    let (can_ask, why) = match decision {
        Decision::TakeOver => (true, "same-program"),
        Decision::Refuse(why) => (false, why.code()),
    };
    StartError::Unresponsive(Stuck {
        pid: Some(peer.pid),
        start: peer.start,
        since: peer.start.and_then(procfs::started_utc),
        exe: peer.exe.as_ref().map(|e| e.path.clone()),
        can_ask,
        why,
    })
}

fn unknown_holder() -> StartError {
    StartError::Unresponsive(Stuck {
        pid: None,
        start: None,
        since: None,
        exe: None,
        can_ask: false,
        why: "unreadable",
    })
}

/// One line to the log of the take-over and to stderr.
fn log(socket: &Path, line: &str) {
    eprintln!("fs-tasks: {line}");
    let mut options = std::fs::OpenOptions::new();
    options.create(true).append(true).mode(0o600);
    if let Ok(mut file) = options.open(socket.with_extension("takeover.log")) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let _ = writeln!(file, "{} {line}", procfs::utc(now));
    }
}

/// Remove the socket file if it is the very file `ino` that the ended process listened on; a socket of a successor
/// (another file) or anything else stays. Returns whether it was removed.
pub(crate) fn remove_if_same(socket: &Path, ino: Option<u64>) -> bool {
    let Some(ino) = ino else { return false };
    match std::fs::symlink_metadata(socket) {
        Ok(meta) if meta.file_type().is_socket() && meta.ino() == ino => {
            std::fs::remove_file(socket).is_ok()
        }
        _ => false,
    }
}

/// Wait until the process is gone, at most `limit`.
async fn gone_within(target: &Target, limit: Duration) -> bool {
    let root = Path::new(PROC);
    let until = Instant::now() + limit;
    loop {
        if procfs::is_gone(root, target.pid, target.start) {
            return true;
        }
        if Instant::now() >= until {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

/// Look at the process once more and accept it for ending: this program (`TakeOver`)
/// only. Another program is never ended, also not with a yes: the failure to hand on says who it is.
fn accepted(target: &Target, uid: u32, own: &Own) -> Result<Peer, StartError> {
    let peer = procfs::peer(target.pid, uid);
    match decide(Some(target.start), &peer, own) {
        Decision::TakeOver => Ok(peer),
        other @ Decision::Refuse(_) => Err(stuck_of(&peer, other)),
    }
}

/// End the process `target` (SIGTERM, then SIGKILL) and remove its socket file. `uid` is what the socket said about
/// the user of the process.
///
/// # Errors
/// The process is not one that may be ended (`Unresponsive` with the reason), or it did not end.
pub(crate) async fn end_process(
    socket: &Path,
    target: &Target,
    uid: u32,
    times: &Times,
) -> Result<(), StartError> {
    let own = procfs::own().ok_or_else(|| failed("cannot read the facts of this program"))?;
    let socket_ino = std::fs::symlink_metadata(socket).ok().map(|m| m.ino());
    accepted(target, uid, &own)?;
    let pid = Pid::from_raw(target.pid).ok_or_else(|| failed("no such process"))?;
    let fd = match pidfd_open(pid, PidfdFlags::empty()) {
        Ok(fd) => fd,
        Err(rustix::io::Errno::SRCH) => return Ok(()),
        Err(e) => return Err(failed(format!("cannot hold process {}: {e}", target.pid))),
    };
    // Looked at again with the descriptor in hand: from now on the number cannot mean another process.
    let peer = accepted(target, uid, &own)?;
    let exe = peer.exe.as_ref().map_or("?", |e| e.path.as_str());
    let line = format!(
        "take-over pid={} start={} uid={} exe={exe} (found through the socket {})",
        target.pid,
        target.start,
        peer.uid,
        socket.display()
    );
    log(socket, &line);
    let _ = pidfd_send_signal(&fd, Signal::TERM);
    let how = if gone_within(target, times.term).await {
        "SIGTERM"
    } else {
        let _ = pidfd_send_signal(&fd, Signal::KILL);
        if !gone_within(target, times.kill).await {
            return Err(failed(format!("process {} does not end", target.pid)));
        }
        "SIGKILL"
    };
    let removed = remove_if_same(socket, socket_ino);
    log(
        socket,
        &format!(
            "ended pid={} by {how}; socket file removed: {removed}",
            target.pid
        ),
    );
    Ok(())
}

/// The silent server at `socket` is looked at and, if it is this program of this user, ended.
///
/// # Errors
/// As for `end_process`; `Unresponsive` when it may not be ended without asking.
pub(crate) async fn free_socket(socket: &Path, times: &Times) -> Result<Freed, StartError> {
    let (pid, uid) = match listener(socket, times.call).await {
        Listener::Gone => return Ok(Freed::Free),
        Listener::Unknown => return Err(unknown_holder()),
        Listener::Process(pid, uid) => (pid, uid),
    };
    let Some(start) = procfs::peer(pid, uid).start else {
        return Err(unknown_holder());
    };
    // A server that started a moment ago may just be opening its store: it gets until `young` to answer.
    let age = procfs::age_secs(start).unwrap_or(f64::MAX);
    let wait = times.young.as_secs_f64() - age;
    if wait > 0.0 {
        let until = Instant::now() + Duration::from_secs_f64(wait);
        while Instant::now() < until {
            match probe(socket, times.call.min(Duration::from_millis(500))).await {
                Probe::Alive => return Ok(Freed::Ready),
                Probe::Absent => return Ok(Freed::Free),
                _ => tokio::time::sleep(Duration::from_millis(100)).await,
            }
        }
    }
    let own = procfs::own().ok_or_else(|| failed("cannot read the facts of this program"))?;
    let peer = procfs::peer(pid, uid);
    match decide(Some(start), &peer, &own) {
        Decision::TakeOver => {
            end_process(socket, &Target { pid, start }, uid, times).await?;
            Ok(Freed::Free)
        }
        other @ Decision::Refuse(_) => Err(stuck_of(&peer, other)),
    }
}

/// What is known about the silent server at `socket`, as the failure to show (nothing is ended).
pub(crate) async fn describe(socket: &Path, times: &Times) -> StartError {
    let Listener::Process(pid, uid) = listener(socket, times.call).await else {
        return unknown_holder();
    };
    let Some(own) = procfs::own() else {
        return unknown_holder();
    };
    let peer = procfs::peer(pid, uid);
    let decision = decide(peer.start, &peer, &own);
    stuck_of(&peer, decision)
}

/// The person said yes: end the process that was shown (same user, still the same process), blocking.
///
/// # Errors
/// As for `end_process`.
pub fn end_confirmed(socket: &Path, stuck: &Stuck) -> Result<(), StartError> {
    let (Some(pid), Some(start)) = (stuck.pid, stuck.start) else {
        return Err(failed("the process is not known: it is not ended"));
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| failed(e.to_string()))?;
    let uid = rustix::process::geteuid().as_raw();
    runtime.block_on(end_process(
        socket,
        &Target { pid, start },
        uid,
        &Times::default(),
    ))
}
