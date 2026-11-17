//! Test providers, conformance rules and the fixed sample data of FreeSynergy tasks (pilot `programm-grundgeruest`,
//! step K0). The frontends develop against [`FakeTasks`]; the core agent owns this crate.
//!
//! Pattern: Fake behind a locked Repository trait (the trait is the folder [`tasks_v1`], byte for byte the locked one).
//!
//! * [`tasks_v1`]: the LOCKED trait folder (SHA-256 66c7cf13...aed085); never edit it, a change is `tasks_v2`.
//! * [`draft`]: the unlocked trait delta `tasks_v2` (D1: stage, columns, note, live feed) and its conformance skeleton
//!   (probe, no lock).
//! * [`FakeTasks`]: in memory, every method and rule; [`EmptyTasks`] is the empty one; [`FakeExchange`] the boundary.
//! * [`sample`]: the fixed set (about 90 tasks in 5 lists, every case of the plan).
//! * [`conformance`]: rules any provider must pass (`run_conformance`, later also for `LocalTasks`).

// Test support: the conformance rules and the sample read as one story each, so long functions and short names
// (`a`, `b`, `t`, `i`, `j`) are deliberate here.
#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::trivially_copy_pass_by_ref
)]

// The locked folder: byte for byte the lock (`.scratch/sperren/tasks_v1.sha256`). It was written for the Rust 2021
// probe and compiled with pedantic lints there; the allows below are the ones the 2024 workspace needs, outside it.
#[rustfmt::skip]
#[allow(
    missing_docs,
    clippy::doc_markdown,
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::module_name_repetitions,
    clippy::large_enum_variant,
    clippy::wildcard_imports
)]
pub mod tasks_v1;

mod blank;
mod clock;
pub mod conformance;
mod decorate;
mod doubles;
pub mod draft;
mod exchange;
mod fake;
mod fake_impl;
mod ids;
mod ops_list;
mod ops_people;
mod ops_progress;
mod ops_relate;
mod ops_task;
mod ops_trash;
mod patch;
mod people;
mod ranking;
mod ready;
mod sample;
mod sample_data;
mod sample_extra;
mod sample_more;
mod sample_people;
mod scope;
mod select;
mod series;
mod state;
mod view;

pub use blank::blank_task;
pub use decorate::{Decorated, Gate, Op};
pub use doubles::{Clock, FailGate, FailingTasks, SlowGate, SlowTasks, forbidden, unavailable};
pub use exchange::{FakeExchange, MEDIA_TYPES};
pub use fake::{DECISIONS, EmptyTasks, FakeTasks, START_MS};
pub use ids::{object_id, object_number, person, reference, try_reference};
pub use people::FakePeople;
pub use sample::{Sample, id_of, occurrence, sample};
pub use sample_people::{KNOWN, Known};
