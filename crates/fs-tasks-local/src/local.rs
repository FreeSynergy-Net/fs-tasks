//! `LocalTasks`: the provider value (store, asking person, clock) and what is not a trait method: open, the trash
//! setting, the deadline run.
//!
//! Pattern: Repository (one provider of the trait) over a small typed store; the trait impl is in `provider`.

use std::sync::{Mutex, MutexGuard};

use fs_objects::v5::envelope::{due, set_trash_days};
use fs_tasks_testdata::tasks_v1::{PersonRef, TaskResult};

use crate::access;
use crate::clock::{Clock, Ticker};
use crate::error::adapt;
use crate::store::{Store, finish};
use crate::trash_ops;
#[cfg(feature = "test-hooks")]
use crate::who::Flaw;
use crate::who::Who;

/// What this run offers (the rest of `CAPABILITIES` is named in `DESIGN.md`: assign, respond, claim, changes, history).
pub const OFFERED: [&str; 17] = [
    "list.read",
    "list.create",
    "list.edit",
    "list.delete",
    "task.read",
    "task.labels",
    "task.create",
    "task.edit",
    "task.progress",
    "task.place",
    "task.checklist",
    "relation.create",
    "relation.delete",
    "dependency.evaluate",
    "task.delete",
    "task.restore",
    "trash.purge",
];

/// The local provider: the typed v5 tables of one store file; the account is the person it belongs to, and it asks
/// unless somebody else is acted as (`act_as`).
pub struct LocalTasks {
    pub(crate) store: Store,
    pub(crate) account: PersonRef,
    pub(crate) acting: Mutex<Option<PersonRef>>,
    pub(crate) agents: Mutex<Vec<PersonRef>>,
    #[cfg(feature = "test-hooks")]
    pub(crate) flaw: Mutex<Option<Flaw>>,
    pub(crate) clock: Ticker,
}

pub(crate) fn locked<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl LocalTasks {
    /// Open the store at `path` (`:memory:` is volatile) and register the `tasks_` domain.
    ///
    /// # Errors
    /// `Unavailable` when the engine cannot open the file; `Invalid` on a clash in the registry.
    pub async fn open(path: &str, account: PersonRef, clock: Clock) -> TaskResult<Self> {
        let clock = Ticker::new(clock);
        let store = Store::open(path, clock.tick()).await?;
        Ok(Self {
            store,
            account,
            acting: Mutex::new(None),
            agents: Mutex::new(Vec::new()),
            #[cfg(feature = "test-hooks")]
            flaw: Mutex::new(None),
            clock,
        })
    }

    /// Who asks now: the account, or the person acted as, and the AIs the provider knows.
    #[must_use]
    pub fn who(&self) -> Who {
        Who {
            me: locked(&self.acting)
                .clone()
                .unwrap_or_else(|| self.account.clone()),
            agents: locked(&self.agents).clone(),
            #[cfg(feature = "test-hooks")]
            flaw: *locked(&self.flaw),
        }
    }

    /// Somebody else asks from now on (`Some`), or the account again (`None`). The provider trusts the caller (the
    /// frontend of a signed-in person is the caller; the tests act as a colleague or an AI).
    pub fn act_as(&self, who: Option<PersonRef>) {
        *locked(&self.acting) = who;
    }

    /// `agent` is an AI (no list of its own, at most `Write`, nobody's owner). The directory of run 2b is the real source.
    pub fn mark_agent(&self, agent: PersonRef) {
        locked(&self.agents).push(agent);
    }

    /// `who` gets `level` on `list` (a grant row; it replaces the level of the owner).
    ///
    /// # Errors
    /// `Unavailable` when the store refuses.
    pub async fn grant(
        &self,
        who: &PersonRef,
        list: fs_tasks_testdata::tasks_v1::ObjectId,
        level: fs_tasks_testdata::tasks_v1::ListAccess,
    ) -> TaskResult<()> {
        let c = self.store.begin().await?;
        let r = access::set_grant(&c, who, list, level).await;
        finish(c, r).await
    }

    /// `who` may only read `list` from now on (the access of the owner is replaced by `Read`).
    ///
    /// # Errors
    /// `Unavailable` when the store refuses.
    pub async fn make_read_only(
        &self,
        who: &PersonRef,
        list: fs_tasks_testdata::tasks_v1::ObjectId,
    ) -> TaskResult<()> {
        let c = self.store.begin().await?;
        let r =
            access::set_grant(&c, who, list, fs_tasks_testdata::tasks_v1::ListAccess::Read).await;
        finish(c, r).await
    }

    /// The trash period in days (the program's setting; the default is 30, at least 1).
    ///
    /// # Errors
    /// `Invalid` for 0 days.
    pub async fn set_trash_days(&self, days: u32) -> TaskResult<()> {
        let c = self.store.begin().await?;
        let r = set_trash_days(&c, i64::from(days)).await.map_err(adapt);
        finish(c, r).await
    }

    /// The store (the tests look at the rows).
    #[must_use]
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// The deadline run: what the trash period ended goes for good, with what hangs on it.
    pub(crate) async fn sweep(&self) -> TaskResult<()> {
        let now = self.clock.peek();
        let nothing = {
            let c = self.store.read().await;
            due(&c, now).await.map_err(adapt)?.is_empty()
        };
        if nothing {
            return Ok(());
        }
        let c = self.store.begin().await?;
        let r = trash_ops::sweep(&c, now).await;
        finish(c, r).await.map(|_| ())
    }
}
