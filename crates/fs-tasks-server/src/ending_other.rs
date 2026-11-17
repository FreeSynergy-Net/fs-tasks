//! Ending a silent server where there is no `/proc` (not Unix): nothing is ended, the person is told.
//!
//! Pattern: Null object — the same functions as `ending`, which always say "unknown holder", so the start stops with
//! a message (never a silent wait) and nobody ends a process from here.

use std::path::Path;

use crate::start::Times;
use crate::startfail::{StartError, Stuck};

/// What `free_socket` found.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Freed {
    /// The server answers now.
    Ready,
    /// The socket is free.
    Free,
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

/// Nothing is ended here.
///
/// # Errors
/// Always `Unresponsive` with an unknown holder.
pub(crate) async fn free_socket(_socket: &Path, _times: &Times) -> Result<Freed, StartError> {
    Err(unknown_holder())
}

/// The failure to show.
pub(crate) async fn describe(_socket: &Path, _times: &Times) -> StartError {
    unknown_holder()
}

/// Nothing is ended here.
///
/// # Errors
/// Always a failure.
pub fn end_confirmed(_socket: &Path, _stuck: &Stuck) -> Result<(), StartError> {
    Err(StartError::Failed(
        "a server that does not answer cannot be ended from here on this system".into(),
    ))
}
