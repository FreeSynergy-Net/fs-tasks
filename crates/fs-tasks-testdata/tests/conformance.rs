//! The conformance rules against the fake (full, without dependency evaluation, on a filled provider) and the
//! `Unsupported` rules against a provider that only has the required methods.

use async_trait::async_trait;
use fs_tasks_testdata::conformance::{run_conformance, run_fixture_conformance, run_unsupported};
use fs_tasks_testdata::tasks_v1::{
    Answer, CAPABILITIES, Capabilities, Change, ChecklistChange, Children, Deleted, Dependency,
    DependencyKind, Error, HistoryEntry, LabelCount, ObjectId, Page, PersonRef, Place, Progress,
    Readiness, Revision, SeriesScope, Task, TaskDraft, TaskError, TaskList, TaskListPatch,
    TaskPatch, TaskQuery, TaskResult, TaskSummary, Tasks, Trash, Utc, not_possible,
};
use fs_tasks_testdata::{FakeTasks, sample};

mod common;
use common::Fx;

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
    async fn create_list(
        &self,
        _name: &str,
        _color: Option<&str>,
        _owner: Option<&PersonRef>,
    ) -> TaskResult<TaskList> {
        not_possible("list.create")
    }
    async fn update_list(
        &self,
        _id: ObjectId,
        _expected: Revision,
        _change: TaskListPatch,
    ) -> TaskResult<TaskList> {
        not_possible("list.edit")
    }
    async fn delete_list(&self, _id: ObjectId, _expected: Revision) -> TaskResult<Deleted> {
        not_possible("list.delete")
    }
    async fn restore_list(&self, _id: ObjectId) -> TaskResult<TaskList> {
        not_possible("list.delete")
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
    async fn labels(
        &self,
        _lists: &[ObjectId],
        _prefix: Option<&str>,
        _after: Option<&Place>,
        _size: usize,
    ) -> TaskResult<Page<LabelCount>> {
        not_possible("task.labels")
    }
    async fn readiness(&self, _ids: &[ObjectId]) -> TaskResult<Vec<(ObjectId, Readiness)>> {
        not_possible("dependency.evaluate")
    }
    async fn create(&self, _draft: TaskDraft) -> TaskResult<Task> {
        not_possible("task.create")
    }
    async fn update(
        &self,
        _id: ObjectId,
        _expected: Revision,
        _change: TaskPatch,
        _scope: SeriesScope,
    ) -> TaskResult<Task> {
        not_possible("task.edit")
    }
    async fn assign(
        &self,
        _id: ObjectId,
        _expected: Revision,
        _who: &PersonRef,
        _scope: SeriesScope,
    ) -> TaskResult<Task> {
        not_possible("task.assign")
    }
    async fn unassign(
        &self,
        _id: ObjectId,
        _expected: Revision,
        _who: &PersonRef,
        _scope: SeriesScope,
    ) -> TaskResult<Task> {
        not_possible("task.assign")
    }
    async fn respond(
        &self,
        _id: ObjectId,
        _expected: Revision,
        _answer: Answer,
        _comment: Option<&str>,
        _scope: SeriesScope,
    ) -> TaskResult<Task> {
        not_possible("task.respond")
    }
    async fn progress(
        &self,
        _id: ObjectId,
        _expected: Revision,
        _to: Progress,
        _children: Children,
        _scope: SeriesScope,
    ) -> TaskResult<Task> {
        not_possible("task.progress")
    }
    async fn place(
        &self,
        _id: ObjectId,
        _expected: Revision,
        _after: Option<ObjectId>,
    ) -> TaskResult<Task> {
        not_possible("task.place")
    }
    async fn checklist(
        &self,
        _id: ObjectId,
        _expected: Revision,
        _change: ChecklistChange,
    ) -> TaskResult<Task> {
        not_possible("task.checklist")
    }
    async fn relate(
        &self,
        _id: ObjectId,
        _expected: Revision,
        _dependency: Dependency,
    ) -> TaskResult<Task> {
        not_possible("relation.create")
    }
    async fn unrelate(
        &self,
        _id: ObjectId,
        _expected: Revision,
        _other_uid: &str,
        _kind: DependencyKind,
    ) -> TaskResult<Task> {
        not_possible("relation.delete")
    }
    async fn claim(&self, _id: ObjectId, _expected: Revision, _until: Utc) -> TaskResult<Task> {
        not_possible("task.claim")
    }
    async fn release(&self, _id: ObjectId, _expected: Revision) -> TaskResult<Task> {
        not_possible("task.claim")
    }
    async fn claim_next(&self, _query: &TaskQuery, _until: Utc) -> TaskResult<Option<Task>> {
        not_possible("task.claim")
    }
    async fn delete(
        &self,
        _id: ObjectId,
        _expected: Revision,
        _scope: SeriesScope,
    ) -> TaskResult<Deleted> {
        not_possible("task.delete")
    }
    async fn restore(&self, _id: ObjectId, _scope: SeriesScope) -> TaskResult<Task> {
        not_possible("task.restore")
    }
    async fn purge(&self, _id: ObjectId) -> TaskResult<()> {
        not_possible("trash.purge")
    }
    async fn history(
        &self,
        _id: ObjectId,
        _after: Option<&Place>,
        _size: usize,
    ) -> TaskResult<Page<HistoryEntry>> {
        not_possible("task.history")
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
    let created = ReadOnly.create_list("x", None, None).await;
    assert!(matches!(
        created,
        Err(TaskError::Common(Error::Unsupported { .. }))
    ));
}
