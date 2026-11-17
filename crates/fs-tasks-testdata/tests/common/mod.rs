//! Shared by the conformance tests: the fixture of the fake (its clock, its "I" card, the people of the sample).

use async_trait::async_trait;
use fs_tasks_testdata::conformance::ConformanceFixture;
use fs_tasks_testdata::tasks_v1::{ObjectId, PersonRef, Tasks, Utc};
use fs_tasks_testdata::{FakeTasks, KNOWN};

/// The fixture of the fake.
pub struct Fx(pub FakeTasks);

#[async_trait]
impl ConformanceFixture for Fx {
    fn tasks(&self) -> &dyn Tasks {
        &self.0
    }
    async fn advance_clock(&self, millis: i64) -> bool {
        self.0
            .set_now(Utc::from_millis(self.0.now().millis() + millis));
        true
    }
    async fn set_me_card(&self, card: Option<PersonRef>) -> bool {
        self.0.set_me_card(card);
        true
    }
    async fn account(&self) -> Option<PersonRef> {
        Some(self.0.account())
    }
    async fn act_as(&self, who: Option<PersonRef>) -> bool {
        self.0.act_as(who);
        true
    }
    async fn my_group(&self) -> Option<PersonRef> {
        Some(known(6))
    }
    async fn foreign_group(&self) -> Option<PersonRef> {
        Some(known(7))
    }
    async fn colleague(&self) -> Option<PersonRef> {
        Some(known(2))
    }
    async fn agent(&self) -> Option<PersonRef> {
        Some(known(8))
    }
    async fn stop_agent(&self, agent: &PersonRef) -> bool {
        self.0.revoke_claims(agent);
        true
    }
    async fn seed_rank(&self, id: ObjectId, rank: &fs_tasks_testdata::tasks_v1::Rank) -> bool {
        self.0.set_rank(id, rank.clone());
        true
    }
    async fn set_trash_days(&self, days: Option<u32>) -> bool {
        self.0.set_trash_days(days);
        true
    }
    async fn read_only(&self, who: &PersonRef, list: ObjectId) -> bool {
        self.0.set_read_only(who, list);
        true
    }
    async fn keep(&self, id: ObjectId, until: Option<Utc>) -> bool {
        self.0.keep(id, until);
        true
    }
}

/// A sample person by number, with its kind.
pub fn known(n: u64) -> PersonRef {
    KNOWN
        .iter()
        .find(|k| k.number == n)
        .map(fs_tasks_testdata::Known::person)
        .expect("a person of the sample")
}
