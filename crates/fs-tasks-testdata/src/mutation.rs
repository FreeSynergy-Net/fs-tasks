//! Mutations of the fake: one deliberately wrong behaviour each, to prove that a conformance rule can fail.
//!
//! Pattern: Mutation testing by switch (`FakeTasks::with_mutation`); a rule that stays green against its mutation proves
//! nothing, so every new rule of the delta has one (`tests/mutants.rs`). Never set in a normal fake.

/// One wrong behaviour of the fake.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mutation {
    /// `checklist` has no limit of `CHECKLIST_MAX` items.
    ChecklistNoLimit,
    /// Moving on from a completion does not take the ticks off the checklist.
    TicksStay,
    /// The claim rule lets everybody take a task that is assigned to a group.
    GroupOpen,
    /// An expired claim still holds.
    ExpiredHolds,
    /// An AI may own a list.
    AiOwns,
    /// "From completion" counts from the old due date, not from the day of the completion.
    RepeatFromDue,
    /// The last completion of a `Count` series does not finish the task.
    NoLastStop,
    /// `purge` ignores a keep.
    PurgeHeld,
    /// A list that goes for good leaves the tasks deleted earlier on their own behind.
    OrphansStay,
    /// No limit of `SUBTASK_DEPTH_MAX` levels.
    NoDepthLimit,
}
