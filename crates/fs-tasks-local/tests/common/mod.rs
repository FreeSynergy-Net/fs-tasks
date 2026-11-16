//! Shared by the tests: the fixture of `LocalTasks` (its clock moves by hand, the account is a fixed person).
//!
//! Pattern: Test fixture (the provider, its clock and the people of the sample behind the conformance hooks).

pub mod deps;

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use async_trait::async_trait;
use fs_objects::v5::codec::get_int;
use fs_tasks_local::{Clock, LocalTasks};
use fs_tasks_testdata::conformance::ConformanceFixture;
use fs_tasks_testdata::tasks_v1::{ObjectId, PersonRef, Progress, TaskDraft, TaskPatch, Tasks};
use fs_tasks_testdata::{KNOWN, Known, START_MS, person};

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
        tasks.mark_agent(known(8));
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

    async fn act_as(&self, who: Option<PersonRef>) -> bool {
        self.tasks.act_as(who);
        true
    }

    /// Another human (sample person 2).
    async fn colleague(&self) -> Option<PersonRef> {
        Some(known(2))
    }

    /// An AI (sample person 8), marked as one in `open`.
    async fn agent(&self) -> Option<PersonRef> {
        Some(known(8))
    }

    /// Only with `test-hooks`; without it the rank rules that need a seeded rank are skipped and named.
    #[cfg(feature = "test-hooks")]
    async fn seed_rank(&self, id: ObjectId, rank: &fs_tasks_testdata::tasks_v1::Rank) -> bool {
        self.tasks.seed_rank(id, rank).await.is_ok()
    }

    async fn read_only(&self, who: &PersonRef, list: ObjectId) -> bool {
        self.tasks.make_read_only(who, list).await.is_ok()
    }

    /// Days until the automatic purge; `None` (= never) needs `trash_auto` of fs-objects v0.0.3 (run 2b), so it stays
    /// refused and the rules for `Manual` are skipped and named.
    async fn set_trash_days(&self, days: Option<u32>) -> bool {
        match days {
            Some(d) => self.tasks.set_trash_days(d).await.is_ok(),
            None => false,
        }
    }
}

/// A person of the sample by number.
pub fn known(n: u64) -> PersonRef {
    KNOWN
        .iter()
        .find(|k| k.number == n)
        .map(Known::person)
        .expect("a person of the sample")
}

/// Move the clock of a provider by hand.
pub fn advance(fx: &LocalFx, millis: i64) {
    fx.now.fetch_add(millis, Ordering::SeqCst);
}

pub fn draft(
    list: fs_tasks_testdata::tasks_v1::ObjectId,
    title: &str,
    rest: TaskPatch,
) -> TaskDraft {
    TaskDraft {
        list,
        uid: None,
        title: title.to_owned(),
        progress: Progress::NeedsAction,
        assignees: Vec::new(),
        checklist: Vec::new(),
        rest,
    }
}

pub async fn count(fx: &LocalFx, table: &str) -> i64 {
    let c = fx.tasks.store().read().await;
    let rows = c
        .query(&format!("SELECT count(*) FROM {table}"), Vec::new())
        .await
        .expect("a count");
    get_int(&rows[0], 0).expect("a number")
}

/// A date of the tests (`2026-11-17`).
pub fn date(s: &str) -> fs_tasks_testdata::tasks_v1::Date {
    use fs_types::BasicValue;
    fs_tasks_testdata::tasks_v1::Date::parse(s).unwrap()
}

/// A timed due value: the local time `at` in the zone `zone`.
pub fn timed(at: &str, zone: &str) -> fs_tasks_testdata::tasks_v1::Due {
    fs_tasks_testdata::tasks_v1::Due::Timed {
        at: fs_tasks_testdata::occurrence(at).unwrap(),
        zone: fs_tasks_testdata::tasks_v1::Zone::iana(zone).unwrap(),
        zone_assumed: false,
    }
}

/// The tasks of `list` in a day range, seen from `view`.
pub fn day_query(
    list: ObjectId,
    view: &str,
    due: Option<fs_tasks_testdata::tasks_v1::DueRange>,
) -> fs_tasks_testdata::tasks_v1::TaskQuery {
    let mut q = deps::list_query(list);
    q.due = due;
    q.view_zone = fs_tasks_testdata::tasks_v1::Zone::iana(view).unwrap();
    q
}
