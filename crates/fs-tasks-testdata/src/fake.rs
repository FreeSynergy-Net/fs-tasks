//! `FakeTasks`: the in-memory provider every other part is tested against first (plan step K0).
//!
//! Pattern: Fake (a test double with working behaviour) behind the `Tasks` Repository trait. It answers every
//! method of the locked trait with the rules of PLAN section 11, keeps a change feed and a log, and has a trash
//! with a 30-day deadline. [`DECISIONS`] lists what it decided where the trait is silent.

use crate::state::State;
use crate::tasks_v1::{ObjectId, PersonRef, Task, TaskList, Utc};
use std::sync::{Mutex, MutexGuard};

/// "Now" of a new fake: 2026-11-16T09:00:00Z (the sample's today).
pub const START_MS: i64 = 1_794_819_600_000;

/// What the fake decided where the locked trait is silent (for the conformance rules and the DESIGN).
pub const DECISIONS: [&str; 10] = [
    "Every write takes one second on the fake's own clock (set_now moves it); the trash deadline is 30 days.",
    "Only daily, weekly, monthly and yearly rules (interval, Never/Count/Until) are expanded; a monthly rule clamps the day to the month's end; a rule with by_* parts is Invalid; Recurrence::Original shows its first occurrence and is Unsupported{task.edit} for This/ThisAndFollowing.",
    "A custom zone ('/...') is UnknownZone everywhere (the fake has no custom zones).",
    "Order::Due uses the current occurrence's due, else start; a timed value sorts before floating ones of the same day, then rank, then id.",
    "TaskQuery::ready = Some(false) lists everything that is not Ready (also Done); text search is case-insensitive over title and description.",
    "AssigneeFilter::Me matches the account AND the 'I' card of the provider; asker is the 'I' card, else the account.",
    "The Move entries of a re-spread are in `changes`, not in a task's `history`; the placed task gets one Move in both.",
    "A subtask deleted with its parent comes back only with it (restore alone is Invalid); a task deleted on its own stays deleted when its list is restored.",
    "assign/unassign/relate repeated change nothing and bump no revision; update with a patch that changes nothing does the same.",
    "TaskExchange: the fake keeps no files, so formats() says No both ways and import/export are Unsupported (the defaults).",
];

/// In-memory `Tasks`.
#[derive(Debug)]
pub struct FakeTasks {
    state: Mutex<State>,
}

/// A fake with no lists and no tasks (the empty case of the plan; Z1).
pub type EmptyTasks = FakeTasks;

impl FakeTasks {
    /// No lists, no tasks; the account is person 1; the clock is at [`START_MS`].
    #[must_use]
    pub fn new() -> Self {
        Self::with_state(State::new(
            crate::ids::person(1),
            Utc::from_millis(START_MS),
            true,
        ))
    }

    pub(crate) fn with_state(state: State) -> Self {
        Self {
            state: Mutex::new(state),
        }
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Same as [`FakeTasks::new`] (Z1).
    #[must_use]
    pub fn empty() -> Self {
        Self::new()
    }

    /// A provider that cannot evaluate dependencies (like a `CalDAV` one): `dependency.evaluate` is off, `readiness`
    /// and `ready = Some(_)` are `Unsupported`, an open task with dependencies is `NotEvaluated`.
    #[must_use]
    pub fn without_evaluation(self) -> Self {
        self.lock().evaluate = false;
        self
    }

    /// The account (Konto-Verweis) of the provider: who asks when there is no "I" card.
    #[must_use]
    pub fn with_account(self, account: PersonRef) -> Self {
        self.lock().account = account;
        self
    }

    /// Set or remove the "I" card of the asking person.
    pub fn set_me_card(&self, card: Option<PersonRef>) {
        self.lock().me_card = card;
    }

    /// The account of the provider.
    #[must_use]
    pub fn account(&self) -> PersonRef {
        self.lock().account.clone()
    }

    /// Set "now" (UTC ms): creation time, log time, trash deadlines. Everything past its deadline is gone at the
    /// next call.
    pub fn set_now(&self, now: Utc) {
        self.lock().now = now;
    }

    /// The fake's clock.
    #[must_use]
    pub fn now(&self) -> Utc {
        self.lock().now
    }

    /// Put a list in as it is (loading the sample).
    pub fn insert_list_raw(&self, list: TaskList) {
        let mut s = self.lock();
        s.n = s.n.max(crate::ids::object_number(list.id));
        s.lists.push(list);
    }

    /// Put a task in as it is: no checks, no log (loading the sample).
    pub fn insert_task_raw(&self, task: Task) {
        let mut s = self.lock();
        s.n = s.n.max(crate::ids::object_number(task.id));
        s.rows.push(crate::state::Row {
            task,
            deleted: None,
            deleted_with: None,
        });
    }

    /// Put a task in as it is and in the trash until `until` (loading the sample).
    pub fn insert_deleted_raw(&self, task: Task, until: Utc) {
        let mut s = self.lock();
        s.n = s.n.max(crate::ids::object_number(task.id));
        s.rows.push(crate::state::Row {
            task,
            deleted: Some(until),
            deleted_with: None,
        });
    }

    /// Give every task a rank in its sequence (list, parent, column) in the order the tasks were inserted.
    pub fn respread_ranks(&self) {
        self.lock().respread_all();
    }

    /// Whether `id` is a task of this provider (living or not): for tests.
    #[must_use]
    pub fn knows(&self, id: ObjectId) -> bool {
        self.lock().at(id).is_some()
    }
}

impl Default for FakeTasks {
    fn default() -> Self {
        Self::new()
    }
}
