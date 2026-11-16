//! Shared by the tests: the fixture of `LocalTasks` (its clock moves by hand, the account is a fixed person).

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use async_trait::async_trait;
use fs_tasks_local::{Clock, LocalTasks};
use fs_tasks_testdata::conformance::ConformanceFixture;
use fs_tasks_testdata::tasks_v1::{PersonRef, Tasks};
use fs_tasks_testdata::{START_MS, person};

/// The provider, the clock behind it, and the account that asks.
pub struct LocalFx {
    pub tasks: LocalTasks,
    pub now: Arc<AtomicI64>,
    pub account: PersonRef,
}

impl LocalFx {
    /// A provider over `path` (`:memory:` or a file), the clock at the day of the sample.
    pub async fn open(path: &str, now: &Arc<AtomicI64>) -> Self {
        let account = person(1);
        let clock = Clock::from_counter(now.clone());
        let tasks = LocalTasks::open(path, account.clone(), clock)
            .await
            .expect("the store opens");
        Self {
            tasks,
            now: now.clone(),
            account,
        }
    }

    /// A volatile provider.
    pub async fn memory() -> Self {
        Self::open(":memory:", &Arc::new(AtomicI64::new(START_MS))).await
    }
}

#[async_trait]
impl ConformanceFixture for LocalFx {
    fn tasks(&self) -> &dyn Tasks {
        &self.tasks
    }

    async fn advance_clock(&self, millis: i64) -> bool {
        self.now.fetch_add(millis, Ordering::SeqCst);
        true
    }

    async fn account(&self) -> Option<PersonRef> {
        Some(self.account.clone())
    }
}

/// Move the clock of a provider by hand.
pub fn advance(fx: &LocalFx, millis: i64) {
    fx.now.fetch_add(millis, Ordering::SeqCst);
}
