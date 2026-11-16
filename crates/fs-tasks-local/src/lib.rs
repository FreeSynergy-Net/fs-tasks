//! `LocalTasks`: the first provider of the `Tasks` trait (`tasks_v1`), over typed fs-objects v5 tables (ADR 0086).
//! Pilot `programm-grundgeruest`, core step K1, run 1 of 2: schema and registry (`tasks_*`), create, read, change,
//! the trash with its deadline (`Restorable` computed here), the subtask tree (PLAN O18), conformance Fake = Local for
//! the rules of those parts. What is not here yet (assignees, progress and series, checklist, dependencies, claim,
//! history and changes, labels, `place`) is named in `.scratch/pilot-tasks/stand.md` and answers `Unsupported`.
//!
//! Pattern: Repository (one provider of the trait) over a small typed store. The v5 layer is the tagged fs-objects
//! (v0.0.2, no lift): its envelope gives revision (compare-and-set), trash, deadline and the final delete; the program
//! adds the tables, the tree rules and the `Restorable` view of a deadline (v0.0.2 has no `restorable_until`, no hold).

#![allow(clippy::missing_errors_doc, clippy::must_use_candidate)]

pub mod clock;
pub mod cur;
pub mod due;
pub mod error;
pub mod lists;
pub mod local;
pub mod patch;
pub mod provider;
pub mod query;
pub mod rank;
pub mod registry;
pub mod store;
pub mod stored;
pub mod summary;
pub mod tables;
pub mod tasks_write;
pub mod trash_ops;
pub mod trash_state;
pub mod tree;
pub mod words;
pub mod zones;

pub use clock::Clock;
pub use local::{LocalTasks, OFFERED};
pub use store::Store;
