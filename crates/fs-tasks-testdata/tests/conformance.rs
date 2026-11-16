//! The conformance rules against the fake (full, without dependency evaluation, on a filled provider) and the
//! `Unsupported` rules against a provider that only has the required methods.

use async_trait::async_trait;
use fs_tasks_testdata::conformance::{
    ConformanceFixture, run_conformance, run_fixture_conformance, run_unsupported,
};
use fs_tasks_testdata::tasks_v1::{
    CAPABILITIES, Capabilities, Change, Error, ObjectId, Page, PersonRef, Place, Task, TaskError,
    TaskList, TaskQuery, TaskResult, TaskSummary, Tasks, Trash, Utc,
};
use fs_tasks_testdata::{FakeTasks, sample};

/// The fixture of the fake: its clock and its "I" card.
struct Fx(FakeTasks);

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
}

#[tokio::test]
async fn the_fake_passes_every_rule() {
    let report = run_fixture_conformance(&Fx(FakeTasks::new())).await;
    assert!(
        report.failures.is_empty(),
        "failed rules: {:#?}",
        report.failures
    );
    assert!(
        report.skipped.is_empty(),
        "skipped rules: {:#?}",
        report.skipped
    );
    // Every rule ran to its end (a setup that fails early would leave far fewer checks).
    assert!(report.checked >= 240, "only {} checks ran", report.checked);
}

#[tokio::test]
async fn the_fake_without_evaluation_passes_its_rules() {
    let failures = run_conformance(&FakeTasks::new().without_evaluation()).await;
    assert!(failures.is_empty(), "failed rules: {failures:#?}");
}

#[tokio::test]
async fn the_rules_work_on_a_filled_provider() {
    let failures = run_conformance(&sample().fake()).await;
    assert!(failures.is_empty(), "failed rules: {failures:#?}");
}

/// A provider with only the required methods: everything else is the default answer.
struct ReadOnly;

#[async_trait]
impl Tasks for ReadOnly {
    fn capabilities(&self) -> Capabilities {
        Capabilities(
            CAPABILITIES
                .iter()
                .map(|c| ((*c).to_owned(), *c == "task.read"))
                .collect(),
        )
    }
    async fn asker(&self) -> TaskResult<PersonRef> {
        Ok(fs_tasks_testdata::person(1))
    }
    async fn lists(&self, _trash: Trash) -> TaskResult<Vec<TaskList>> {
        Ok(Vec::new())
    }
    async fn get(&self, _id: ObjectId) -> TaskResult<Option<Task>> {
        Ok(None)
    }
    async fn get_by_uid(&self, _uid: &str) -> TaskResult<Option<Task>> {
        Ok(None)
    }
    async fn list(
        &self,
        _q: &TaskQuery,
        _after: Option<&Place>,
        _size: usize,
    ) -> TaskResult<Page<TaskSummary>> {
        Ok(Page {
            items: Vec::new(),
            next: None,
        })
    }
    async fn changes(&self, _since: Option<&Place>, _size: usize) -> TaskResult<Page<Change>> {
        Ok(Page {
            items: Vec::new(),
            next: Some(Place("0".to_owned())),
        })
    }
}

#[tokio::test]
async fn a_provider_with_only_the_required_methods_says_unsupported() {
    let failures = run_unsupported(&ReadOnly).await;
    assert!(failures.is_empty(), "failed rules: {failures:#?}");
    let created = ReadOnly.create_list("x", None).await;
    assert!(matches!(
        created,
        Err(TaskError::Common(Error::Unsupported { .. }))
    ));
}
