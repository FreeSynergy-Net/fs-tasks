//! `fs-tasks-server`: the contract `tasks.capnp` over a Unix socket, in front of any provider of
//! `TaskItems` and `TaskFeed`. It only translates: no task logic lives here.
//!
//! Interface status: draft (ADR 0045).
//!
//! Pattern: Facade — this file only declares modules and re-exports.

#![deny(missing_docs)]
#![warn(clippy::pedantic)]
// The generated `Server` traits are async for every method, also those that never wait.
#![allow(clippy::missing_errors_doc, clippy::unused_async_trait_impl)]

mod client;
mod cursor;
mod datadir;
mod datadir_start;
mod datadir_text;
mod early;
mod edit_if;
#[cfg(unix)]
mod ending;
#[cfg(not(unix))]
#[path = "ending_other.rs"]
mod ending;
#[cfg(all(test, unix))]
mod ending_tests;
mod errors;
mod facts;
mod feed;
mod handshake;
#[cfg(all(test, unix))]
mod handshake_tests;
mod host;
mod items_service;
mod paging;
#[cfg(unix)]
mod procfs;
mod program;
mod serverlog;
mod start;
mod startfail;
#[cfg(test)]
mod startfail_tests;
mod takeover;
#[cfg(test)]
mod takeover_tests;
mod tasks_service;
mod wire;
mod wire_caps;

pub use client::{Changes, RemoteItems};
pub use datadir::{Action, Fault, Meta, Tightened, create_private_dir, decide, tighten};
pub use datadir_start::{check_ancestors, correct};
pub use datadir_text::{Lang, close_word, fault_text, start_text, tightened_text, window_words};
pub use ending::end_confirmed;
pub use host::{Hosted, host};
pub use program::serve_command;
pub use start::{Spawn, Times, ensure_server, ensure_server_with};
pub use startfail::{StartError, Stuck};
pub use takeover::{Decision, Image, Own, Peer, Why, decide as decide_takeover};
pub use tasks_service::TasksServer;
