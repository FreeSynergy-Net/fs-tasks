//! The lists of the tasks program: where a task stands, how its due date stands, which list holds it, what a
//! new task takes over from the view it is typed in.
//!
//! Pattern: Specification + Strategy — pure functions of a task, its readiness and today; no window, no store.
//! Sorted here from `fs-tasks-app` (`plan_view/look.rs`, `next_data.rs`, `defaults.rs`) so every frontend asks the
//! same questions ("Today", "Ready", "Blocked"). Settable later (not yet): see `DESIGN.md`, section Lists, for the
//! rule -> setting-key table.

mod defaults;
mod standing;

pub use defaults::{NewTaskDefaults, View, defaults_for, fits, landing};
pub use standing::{
    Standing, Urgency, in_list, is_inbox, is_open, label_list, people_of, standing_of, urgency,
};
