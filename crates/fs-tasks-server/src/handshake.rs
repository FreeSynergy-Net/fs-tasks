//! The handshake with the server at a socket, with a time limit on every step.
//!
//! Pattern: Probe — connect and ask `Control.version` (fs-interfaces `rpc`, ADR 0065), each step within `call`. Up
//! to 0.0.9 only `connect()` was tried: a server that had bound the socket but did not answer (the kernel takes the
//! connection into the backlog) counted as "there", and the window waited for ever. Now "there" means: it answered.

use std::io;
use std::path::Path;
use std::time::Duration;

use fs_interfaces::common_capnp::control;
use fs_interfaces::rpc::connect;
use tokio::time::timeout;

/// The name the server of this program gives (`ServerIdentity`).
pub(crate) const PROGRAM: &str = "fs-tasks";

/// What the handshake found.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Probe {
    /// A server answered: it is there. (One that does not know `Control` is alive as well: the window's own server
    /// has none.)
    Alive,
    /// Nobody listens, or the socket is gone.
    Absent,
    /// Something listens (or holds the socket) and did not answer within the time.
    Silent,
    /// The server answered and says it is another program.
    Other(String),
    /// Something else is wrong, for example another user's socket.
    Failed(String),
}

fn absent(kind: io::ErrorKind) -> bool {
    matches!(
        kind,
        io::ErrorKind::NotFound
            | io::ErrorKind::ConnectionRefused
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::BrokenPipe
    )
}

/// Ask the server at `socket` who it is; every step may take `call` at most. Must run on a `LocalSet`.
pub(crate) async fn probe(socket: &Path, call: Duration) -> Probe {
    let connection = match timeout(call, connect::<control::Client>(socket)).await {
        Err(_) => return Probe::Silent,
        Ok(Err(e)) if absent(e.kind()) => return Probe::Absent,
        Ok(Err(e)) => return Probe::Failed(format!("{}: {e}", socket.display())),
        Ok(Ok(connection)) => connection,
    };
    let Ok(answer) = timeout(call, connection.client.version_request().send().promise).await else {
        return Probe::Silent;
    };
    match answer {
        Ok(response) => {
            let program = response
                .get()
                .and_then(control::version_results::Reader::get_program)
                .ok()
                .and_then(|t| t.to_str().ok().map(str::to_owned))
                .unwrap_or_default();
            if program.is_empty() || program == PROGRAM {
                Probe::Alive
            } else {
                Probe::Other(program)
            }
        }
        // A server without `Control` says "not implemented": it answers, so it is there.
        Err(e)
            if e.kind == capnp::ErrorKind::Unimplemented || e.extra.contains("not implemented") =>
        {
            Probe::Alive
        }
        Err(e) if e.kind == capnp::ErrorKind::Disconnected => Probe::Absent,
        Err(e) => Probe::Failed(format!("{}: {e}", socket.display())),
    }
}
