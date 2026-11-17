//! Opus review K0c-3, C1: a setup step of a rule that fails must be a loud failure, never a quiet end of the rule (a
//! rule that stops quietly passes for a provider that cannot set it up, and the rule then proves nothing).
//!
//! Pattern: Test double (a fake behind a failing decorator, switched on at a known point of the run).

#[allow(dead_code)]
mod common;

use std::sync::Arc;
use std::sync::Mutex;

use async_trait::async_trait;
use common::known;
use fs_tasks_testdata::conformance::{ConformanceFixture, Options, run_fixture_conformance_with};
use fs_tasks_testdata::tasks_v1::{ObjectId, PersonRef, Tasks, Utc};
use fs_tasks_testdata::{FailingTasks, FakeTasks, Op, unavailable};

/// A fake that fails `fail`: from the start, or from the moment the asker is set to the same person twice in a row (in
/// the leak rules that is the setup of the purged list, right after the colleague purged a task).
struct Loud {
    fake: Arc<FakeTasks>,
    failing: FailingTasks,
    fail: Op,
    last: Mutex<Option<PersonRef>>,
    on_repeat: bool,
}

impl Loud {
    fn new(fail: Op, on_repeat: bool) -> Self {
        let fake = Arc::new(FakeTasks::new());
        let failing = FailingTasks::new(fake.clone());
        if !on_repeat {
            failing.set_failing(fail, unavailable("gone"));
        }
        Self {
            fake,
            failing,
            fail,
            last: Mutex::new(None),
            on_repeat,
        }
    }
}

#[async_trait]
impl ConformanceFixture for Loud {
    fn tasks(&self) -> &dyn Tasks {
        &self.failing
    }
    async fn advance_clock(&self, millis: i64) -> bool {
        self.fake
            .set_now(Utc::from_millis(self.fake.now().millis() + millis));
        true
    }
    async fn account(&self) -> Option<PersonRef> {
        Some(self.fake.account())
    }
    async fn act_as(&self, who: Option<PersonRef>) -> bool {
        let mut last = self.last.lock().unwrap();
        if self.on_repeat && who.is_some() && *last == who {
            self.failing.set_failing(self.fail, unavailable("gone"));
        }
        last.clone_from(&who);
        drop(last);
        self.fake.act_as(who);
        true
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
    async fn read_only(&self, _who: &PersonRef, _list: ObjectId) -> bool {
        false
    }
}

async fn failures(fx: &Loud) -> Vec<String> {
    let options = Options {
        heavy: false,
        skip_rank: true,
        partial: false,
    };
    Box::pin(run_fixture_conformance_with(fx, options))
        .await
        .failures
}

#[tokio::test]
async fn a_delete_that_fails_in_the_restore_depth_setup_is_a_failure() {
    let found = failures(&Loud::new(Op::Delete, false)).await;
    assert!(
        found
            .iter()
            .any(|f| f.contains("restore depth setup: delete the subtree")),
        "the quiet end is back: {found:#?}"
    );
}

#[tokio::test]
async fn a_create_list_that_fails_in_the_purged_list_setup_is_a_failure() {
    let found = failures(&Loud::new(Op::CreateList, true)).await;
    assert!(
        found
            .iter()
            .any(|f| f.contains("leak setup: create_list for the purged list")),
        "the quiet end is back: {found:#?}"
    );
}
