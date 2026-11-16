//! `LocalTasks`: the first provider of the `Tasks` trait (`tasks_v1`), over typed fs-objects v5 tables (ADR 0086).
//! Pilot `programm-grundgeruest`, core step K1, run 2a of 2b: schema and registry (`tasks_*`, three steps), create,
//! read, change, the trash with its deadline (`Restorable` computed here), the subtask tree (PLAN O18), labels, the
//! hand order (`place`), progress (without a series), the checklist, who sees which list (owner, grant, AI), `list`
//! with the filters, the order and the page in SQL, conformance Fake = Local. What is not here yet (assignees and
//! the claim, dependencies, series, alerts and links, history and changes, `Held`/`Manual`) is named in
//! `.scratch/pilot-tasks/stand.md` ("Rest fuer Lauf 2b") and answers `Unsupported`.
//!
//! Pattern: Repository (one provider of the trait) over a small typed store. The v5 layer is the tagged fs-objects
//! (v0.0.2, no lift): its envelope gives revision (compare-and-set), trash, deadline and the final delete; the program
//! adds the tables, the tree rules and the `Restorable` view of a deadline (v0.0.2 has no `restorable_until`, no hold).

#![allow(clippy::missing_errors_doc, clippy::must_use_candidate)]

pub mod access;
pub mod checklist;
pub mod checklist_rows;
pub mod clock;
pub mod cur;
pub mod due;
pub mod error;
pub mod labels;
pub mod lists;
pub mod local;
pub mod patch;
pub mod place;
pub mod progress;
pub mod provider;
pub mod query;
pub mod rank;
pub mod reads;
pub mod registry;
pub mod store;
pub mod stored;
pub mod summary;
pub mod tables;
pub mod tasks_write;
pub mod trash_ops;
pub mod trash_state;
pub mod tree;
pub mod who;
pub mod words;
pub mod zones;

pub use clock::Clock;
pub use local::{LocalTasks, OFFERED};
pub use store::Store;
pub use who::Flaw;
