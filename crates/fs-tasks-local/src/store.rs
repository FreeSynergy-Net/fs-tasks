//! The store of tasks: the layer's store with the `tasks_` domain registered, and the unit of work.
//!
//! Pattern: Adapter (the layer's `Store` and `Conn`, errors in the trait's). A unit of work is `begin`, the work, and
//! `finish`: commit on `Ok`, rollback on `Err` (the work's own error survives the rollback).

use fs_objects::v5;
use fs_tasks_testdata::tasks_v1::TaskResult;

use crate::error::adapt;
use crate::registry::DOMAIN;

/// The store (one file, one program: tasks).
pub struct Store(v5::Store);

impl Store {
    /// Open (creating if needed) the store at `path` (`:memory:` is volatile) and register the `tasks_` domain; the
    /// layer applies the steps that are missing.
    ///
    /// # Errors
    /// `Unavailable` when the engine cannot open the file or a step fails; `Invalid` on a clash in the registry.
    pub async fn open(path: &str, now: i64) -> TaskResult<Self> {
        let store = v5::Store::open(path, now).await.map_err(adapt)?;
        store.register_domain(&DOMAIN, now).await.map_err(adapt)?;
        Ok(Self(store))
    }

    /// Lock the connection and open a transaction (`BEGIN IMMEDIATE`).
    ///
    /// # Errors
    /// `Unavailable` when the engine refuses.
    pub async fn begin(&self) -> TaskResult<v5::Conn<'_>> {
        self.0.begin().await.map_err(adapt)
    }

    /// Lock the connection for reading (no transaction).
    pub async fn read(&self) -> v5::Conn<'_> {
        self.0.read().await
    }
}

/// End a unit of work: commit on `Ok`, roll back on `Err`.
///
/// # Errors
/// The error of `r`, or of the commit.
pub async fn finish<T>(c: v5::Conn<'_>, r: TaskResult<T>) -> TaskResult<T> {
    match r {
        Ok(v) => c.finish(Ok(v)).await.map_err(adapt),
        Err(e) => {
            let _ = c
                .finish::<()>(Err(v5::Error::Unavailable("rolled back".to_owned())))
                .await;
            Err(e)
        }
    }
}
