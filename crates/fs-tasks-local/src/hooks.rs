//! The hooks of the tests (feature `test-hooks`, never in the delivered provider): a deliberate fault of the rights
//! and a task put at an exact rank.
//!
//! Pattern: Test double switch. The conformance rules and the mutants need states the trait cannot produce (a seeded
//! rank of 255 characters) and faults that must turn a rule red; without the feature none of this exists.

use fs_objects::v5::codec::{id_value, text};
use fs_tasks_testdata::tasks_v1::{ObjectId, Rank, TaskResult};

use crate::error::adapt;
use crate::local::{LocalTasks, locked};
use crate::store::finish;
use crate::who::Flaw;

impl LocalTasks {
    /// Put a task at an exact rank without a new revision (the rank rules start the spread by seeding a rank of 255
    /// characters).
    ///
    /// # Errors
    /// `Unavailable` when the store refuses.
    pub async fn seed_rank(&self, id: ObjectId, rank: &Rank) -> TaskResult<()> {
        let c = self.store.begin().await?;
        let r = c
            .exec(
                "UPDATE tasks_task SET rank = ? WHERE id = ?",
                vec![text(rank.as_str()), id_value(id)],
            )
            .await
            .map(|_| ())
            .map_err(adapt);
        finish(c, r).await
    }

    /// Switch a deliberate fault on or off (the mutants of the tests of the rights).
    pub fn set_flaw(&self, flaw: Option<Flaw>) {
        *locked(&self.flaw) = flaw;
    }
}
