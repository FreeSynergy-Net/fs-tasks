//! `FailingTasks`, `SlowTasks` and `HeldTasks` (in `hold.rs`): the loading, error, partly broken and disconnected states
//! of the plan (Z2 to Z5).
//!
//! Pattern: Decorator over `dyn Tasks`; the failure and the wait are configured per method. The wait of `SlowTasks` is
//! virtual: it moves the clock of the fake (`Clock`), nobody sleeps for real, so a test of "loading for 3 s" is
//! instant and exact. A frontend that must SHOW the loading state needs a call that stays open: that is `HeldTasks`.

use crate::decorate::{Decorated, Gate, Op};
use crate::fake::FakeTasks;
use crate::tasks_v1::{Error, ObjectId, TaskError, TaskResult, Tasks, Utc};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

/// A clock a slow provider can move forward.
pub trait Clock: Send + Sync {
    /// Move the clock by `ms` milliseconds.
    fn advance(&self, ms: i64);
}

impl Clock for FakeTasks {
    fn advance(&self, ms: i64) {
        self.set_now(Utc::from_millis(self.now().millis() + ms));
    }
}

fn locked<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The error a failing double answers with: `Unavailable` (the service is gone), `Forbidden` (not allowed for the asker).
#[must_use]
pub fn unavailable(why: &str) -> TaskError {
    TaskError::Common(Error::Unavailable(why.to_owned()))
}

/// See [`unavailable`]; `capability` is the capability id of the refused method.
#[must_use]
pub fn forbidden(capability: &str) -> TaskError {
    TaskError::Common(Error::Forbidden {
        capability: capability.to_owned(),
    })
}

impl<G: Gate> Decorated<G> {
    /// Wrap `inner` behind any gate.
    #[must_use]
    pub fn with_gate(inner: Arc<dyn Tasks>, gate: G) -> Self {
        Self { inner, gate }
    }
}

/// The gate of [`FailingTasks`].
#[derive(Default)]
pub struct FailGate {
    all: Mutex<Option<TaskError>>,
    ops: Mutex<HashMap<Op, TaskError>>,
    lists: Mutex<HashMap<ObjectId, TaskError>>,
}

#[async_trait]
impl Gate for FailGate {
    async fn pass(&self, op: Op) -> TaskResult<()> {
        if let Some(e) = locked(&self.ops).get(&op) {
            return Err(e.clone());
        }
        locked(&self.all).clone().map_or(Ok(()), Err)
    }

    async fn pass_list(&self, lists: &[ObjectId]) -> TaskResult<()> {
        let hit = {
            let broken = locked(&self.lists);
            // No list named = all lists (Z4): a broken one is among them.
            if lists.is_empty() {
                broken.values().next().cloned()
            } else {
                lists.iter().find_map(|l| broken.get(l).cloned())
            }
        };
        if let Some(e) = hit {
            return Err(e);
        }
        self.pass(Op::List).await
    }
}

/// A provider that fails: every method, chosen methods, or `list` of chosen lists.
pub type FailingTasks = Decorated<FailGate>;

impl FailingTasks {
    /// Wrap `inner`; nothing fails yet.
    #[must_use]
    pub fn new(inner: Arc<dyn Tasks>) -> Self {
        Self {
            inner,
            gate: FailGate::default(),
        }
    }

    /// Every method answers `error` (Z3: error; Z5: disconnected).
    #[must_use]
    pub fn failing_all(self, error: TaskError) -> Self {
        *locked(&self.gate.all) = Some(error);
        self
    }

    /// The method `op` answers `error`.
    #[must_use]
    pub fn failing(self, op: Op, error: TaskError) -> Self {
        locked(&self.gate.ops).insert(op, error);
        self
    }

    /// `list` answers `error` when `list` is among the lists asked for (Z4: one list is broken).
    #[must_use]
    pub fn failing_list(self, list: ObjectId, error: TaskError) -> Self {
        locked(&self.gate.lists).insert(list, error);
        self
    }

    /// From now on the method `op` answers `error` (a failure that starts in the middle of a run).
    pub fn set_failing(&self, op: Op, error: TaskError) {
        locked(&self.gate.ops).insert(op, error);
    }

    /// Everything works again (the service came back, Z5).
    pub fn heal(&self) {
        *locked(&self.gate.all) = None;
        locked(&self.gate.ops).clear();
        locked(&self.gate.lists).clear();
    }
}

/// The gate of [`SlowTasks`].
pub struct SlowGate {
    clock: Arc<dyn Clock>,
    every: i64,
    ops: Mutex<HashMap<Op, i64>>,
    waited: AtomicI64,
}

#[async_trait]
impl Gate for SlowGate {
    async fn pass(&self, op: Op) -> TaskResult<()> {
        let ms = locked(&self.ops).get(&op).copied().unwrap_or(self.every);
        self.clock.advance(ms);
        self.waited.fetch_add(ms, Ordering::Relaxed);
        Ok(())
    }
}

/// A provider that takes time: every method `every_ms` of the fake's clock, chosen methods another time (Z2: loading).
pub type SlowTasks = Decorated<SlowGate>;

impl SlowTasks {
    /// Wrap `inner`; each call moves `clock` by `every_ms` (the clock is usually the `FakeTasks` inside).
    #[must_use]
    pub fn new(inner: Arc<dyn Tasks>, clock: Arc<dyn Clock>, every_ms: i64) -> Self {
        Self {
            inner,
            gate: SlowGate {
                clock,
                every: every_ms,
                ops: Mutex::new(HashMap::new()),
                waited: AtomicI64::new(0),
            },
        }
    }

    /// A slow `FakeTasks`: its own clock is the one that moves.
    #[must_use]
    pub fn of_fake(fake: Arc<FakeTasks>, every_ms: i64) -> Self {
        Self::new(fake.clone(), fake, every_ms)
    }

    /// The method `op` takes `ms` instead of the default.
    #[must_use]
    pub fn taking(self, op: Op, ms: i64) -> Self {
        locked(&self.gate.ops).insert(op, ms);
        self
    }

    /// All the time the calls have taken so far (ms of the fake's clock).
    #[must_use]
    pub fn waited_ms(&self) -> i64 {
        self.gate.waited.load(Ordering::Relaxed)
    }
}
