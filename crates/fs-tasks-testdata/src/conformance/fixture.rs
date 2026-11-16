//! `ConformanceFixture`: how a provider under test is put into the states the rules need.
//!
//! Pattern: Strategy / Test fixture (the rules are written once; each provider says how to move its clock or set
//! the "I" card, so the same rules run against the fake and later against `LocalTasks`).

use crate::tasks_v1::{PersonRef, Tasks};
use async_trait::async_trait;

/// The provider under test plus the mutators behind it. A mutator the provider cannot offer keeps its default
/// (`false` / `None`); the rules that need it are skipped.
#[async_trait]
pub trait ConformanceFixture: Send + Sync {
    /// The provider under test.
    fn tasks(&self) -> &dyn Tasks;

    /// Move the provider's clock forward (trash deadlines and claims run out). `true` when done.
    async fn advance_clock(&self, _millis: i64) -> bool {
        false
    }

    /// Set (`Some`) or remove (`None`) the "I" card of the asking person. `true` when done.
    async fn set_me_card(&self, _card: Option<PersonRef>) -> bool {
        false
    }

    /// The account (Konto-Verweis) of the provider: who asks when there is no "I" card.
    async fn account(&self) -> Option<PersonRef> {
        None
    }
}
