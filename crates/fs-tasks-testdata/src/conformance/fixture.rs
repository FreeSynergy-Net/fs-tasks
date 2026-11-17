//! `ConformanceFixture`: how a provider under test is put into the states the rules need.
//!
//! Pattern: Strategy / Test fixture (the rules are written once; each provider says how to move its clock or set
//! the "I" card, so the same rules run against the fake and later against `LocalTasks`).

use crate::tasks_v1::{ObjectId, PersonRef, Tasks, Utc};
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

    /// Somebody else asks from now on (`Some`: a colleague, an AI; `None`: the account again). `true` when done.
    async fn act_as(&self, _who: Option<PersonRef>) -> bool {
        false
    }

    /// The account (Konto-Verweis) of the provider: who asks when there is no "I" card.
    async fn account(&self) -> Option<PersonRef> {
        None
    }

    /// A group the asking person is an effective member of (for the claim rule and `AssigneeFilter::MyGroups`).
    async fn my_group(&self) -> Option<PersonRef> {
        None
    }

    /// A group the asking person is NOT in (the claim rule says `Forbidden`).
    async fn foreign_group(&self) -> Option<PersonRef> {
        None
    }

    /// Another human the provider knows (act as them with `set_me_card`; they hold a claim the asker meets).
    async fn colleague(&self) -> Option<PersonRef> {
        None
    }

    /// An AI the provider knows as AI (act as it with `set_me_card`: no list ownership, no human groups).
    async fn agent(&self) -> Option<PersonRef> {
        None
    }

    /// "STOP KI": the rights of the AI are withdrawn; every claim of it ends at once. `true` when done.
    async fn stop_agent(&self, _agent: &PersonRef) -> bool {
        false
    }

    /// `who` may only read the list from now on (access `Read`: seeing, not writing). `true` when done.
    async fn read_only(&self, _who: &PersonRef, _list: ObjectId) -> bool {
        false
    }

    /// Put a task at an exact rank WITHOUT a new revision (the rank rules start the spread by seeding a rank of 255
    /// characters instead of creating thousands of tasks). `true` when done.
    async fn seed_rank(&self, _id: ObjectId, _rank: &crate::tasks_v1::Rank) -> bool {
        false
    }

    /// The trash setting: days until the automatic purge, `None` = never automatically. `true` when done.
    async fn set_trash_days(&self, _days: Option<u32>) -> bool {
        false
    }

    /// Keep a task or list (`keep_until`; `None` = without an end). `true` when done.
    async fn keep(&self, _id: ObjectId, _until: Option<Utc>) -> bool {
        false
    }
}
