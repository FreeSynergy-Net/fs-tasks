//! `LocalTasks`: the provider value (store, asking person, clock) and what is not a trait method: open, the trash
//! setting, the deadline run.
//!
//! Pattern: Repository (one provider of the trait) over a small typed store; the trait impl is in `provider`.

use fs_objects::v5::envelope::{due, set_trash_days};
use fs_tasks_testdata::tasks_v1::{PersonRef, TaskResult};

use crate::clock::{Clock, Ticker};
use crate::error::adapt;
use crate::store::{Store, finish};
use crate::trash_ops;

/// What this run offers (the rest of `CAPABILITIES` is run 2).
pub const OFFERED: [&str; 10] = [
    "list.read",
    "list.create",
    "list.edit",
    "list.delete",
    "task.read",
    "task.create",
    "task.edit",
    "task.delete",
    "task.restore",
    "trash.purge",
];

/// The local provider: the typed v5 tables of one store file; `actor` is the asking person (the account).
pub struct LocalTasks {
    pub(crate) store: Store,
    pub(crate) actor: PersonRef,
    pub(crate) clock: Ticker,
}

impl LocalTasks {
    /// Open the store at `path` (`:memory:` is volatile) and register the `tasks_` domain.
    ///
    /// # Errors
    /// `Unavailable` when the engine cannot open the file; `Invalid` on a clash in the registry.
    pub async fn open(path: &str, actor: PersonRef, clock: Clock) -> TaskResult<Self> {
        let clock = Ticker::new(clock);
        let store = Store::open(path, clock.tick()).await?;
        Ok(Self {
            store,
            actor,
            clock,
        })
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
