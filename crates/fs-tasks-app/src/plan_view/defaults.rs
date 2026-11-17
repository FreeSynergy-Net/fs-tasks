//! What a new task takes over from the view it is typed in: the rules live in `fs-tasks-core` (`lists::defaults`);
//! this file only names them for the plan views.
//!
//! Pattern: Facade — a re-export of the rules in `fs-tasks-core`, so the plan views keep their import path.

pub use fs_tasks_core::lists::{NewTaskDefaults, View, defaults_for, fits, landing};
