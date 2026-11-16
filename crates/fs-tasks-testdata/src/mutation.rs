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
    /// `history`, `changes` and `purge` ignore whether the asker may see the list (a leak towards an AI).
    LeakTrash,
    /// Everybody sees the tasks of every list.
    SeesAll,
    /// `purge` of a parent also deletes a kept subtask that went with it.
    PurgeTakesKept,
    /// `claim` answers `Claimed` before it checks the claim rule (tells an outsider who holds a task).
    ClaimedFirst,
    /// `release` of an expired claim skips the claim rule.
    ReleaseOpen,
    /// A completion may be undone at any exception, not only at the latest one.
    UndoAny,
    /// `claim_next` does not ask whether a task is ready.
    NextIgnoresReady,
    /// `restore` says `Invalid` before it asks for the right to write.
    RestoreInvalidFirst,
    /// `restore` does not check the depth of the tree it brings back.
    RestoreNoDepth,
    /// The deadline of a deletion is a fixed date, not the time of the delete plus the trash period.
    FixedDeadline,
    /// A delete has no deadline of its own: the state is `Manual`, not `Until` (the deadline rule must fail, not skip).
    NoDeadline,
    /// A rank that runs out on top is an error, not a spread.
    TopNoSpread,
    /// The spread of a column leaves the revisions of the tasks it rewrote as they were.
    SpreadSilent,
}
