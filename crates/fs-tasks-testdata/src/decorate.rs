//! The decorator both doubles share: every method of `Tasks` goes through one gate, then to the provider inside.
//!
//! Pattern: Decorator (over `dyn Tasks`) with a Strategy for the gate (`FailGate`, `SlowGate`).

use crate::tasks_v1::{
    Answer, Capabilities, Change, Children, Deleted, Dependency, DependencyKind, HistoryEntry,
    LabelCount, ObjectId, Page, PersonRef, Place, Progress, Readiness, Revision, SeriesScope, Task,
    TaskDraft, TaskList, TaskPatch, TaskQuery, TaskResult, TaskSummary, Tasks, Trash, Utc,
};
use async_trait::async_trait;
use std::sync::Arc;

/// The methods of `Tasks` a double can fail or slow down (`capabilities` is a plain read and never gated).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Op {
    Asker,
    Lists,
    CreateList,
    UpdateList,
    DeleteList,
    RestoreList,
    Get,
    GetByUid,
    List,
    Labels,
    Readiness,
    Create,
    Update,
    Assign,
    Unassign,
    Respond,
    Progress,
    Place,
    Relate,
    Unrelate,
    Claim,
    Release,
    Delete,
    Restore,
    History,
    Changes,
}

/// What a gate decides before a call: let it pass (maybe after a virtual wait) or answer with an error.
pub trait Gate: Send + Sync {
    /// # Errors
    /// The error the call answers with instead of reaching the provider inside.
    fn pass(&self, op: Op) -> TaskResult<()>;

    /// Same for `list`, which knows the lists asked for (a partly broken provider, plan Z4).
    ///
    /// # Errors
    /// As [`Gate::pass`].
    fn pass_list(&self, lists: &[ObjectId]) -> TaskResult<()> {
        let _ = lists;
        self.pass(Op::List)
    }
}

/// A provider with a gate in front.
pub struct Decorated<G: Gate> {
    pub(crate) inner: Arc<dyn Tasks>,
    pub(crate) gate: G,
}

impl<G: Gate> std::fmt::Debug for Decorated<G> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Decorated(..)")
    }
}

#[async_trait]
impl<G: Gate> Tasks for Decorated<G> {
    fn capabilities(&self) -> Capabilities {
        self.inner.capabilities()
    }
    async fn asker(&self) -> TaskResult<PersonRef> {
        self.gate.pass(Op::Asker)?;
        self.inner.asker().await
    }
    async fn lists(&self, trash: Trash) -> TaskResult<Vec<TaskList>> {
        self.gate.pass(Op::Lists)?;
        self.inner.lists(trash).await
    }
    async fn create_list(&self, name: &str, color: Option<&str>) -> TaskResult<TaskList> {
        self.gate.pass(Op::CreateList)?;
        self.inner.create_list(name, color).await
    }
    async fn update_list(
        &self,
        id: ObjectId,
        expected: Revision,
        name: Option<&str>,
        color: Option<Option<&str>>,
    ) -> TaskResult<TaskList> {
        self.gate.pass(Op::UpdateList)?;
        self.inner.update_list(id, expected, name, color).await
    }
    async fn delete_list(&self, id: ObjectId, expected: Revision) -> TaskResult<Deleted> {
        self.gate.pass(Op::DeleteList)?;
        self.inner.delete_list(id, expected).await
    }
    async fn restore_list(&self, id: ObjectId) -> TaskResult<TaskList> {
        self.gate.pass(Op::RestoreList)?;
        self.inner.restore_list(id).await
    }
    async fn get(&self, id: ObjectId) -> TaskResult<Option<Task>> {
        self.gate.pass(Op::Get)?;
        self.inner.get(id).await
    }
    async fn get_by_uid(&self, uid: &str) -> TaskResult<Option<Task>> {
        self.gate.pass(Op::GetByUid)?;
        self.inner.get_by_uid(uid).await
    }
    async fn list(
        &self,
        query: &TaskQuery,
        after: Option<&Place>,
        size: usize,
    ) -> TaskResult<Page<TaskSummary>> {
        self.gate.pass_list(&query.lists)?;
        self.inner.list(query, after, size).await
    }
    async fn labels(
        &self,
        lists: &[ObjectId],
        prefix: Option<&str>,
        after: Option<&Place>,
        size: usize,
    ) -> TaskResult<Page<LabelCount>> {
        self.gate.pass(Op::Labels)?;
        self.inner.labels(lists, prefix, after, size).await
    }
    async fn readiness(&self, ids: &[ObjectId]) -> TaskResult<Vec<(ObjectId, Readiness)>> {
        self.gate.pass(Op::Readiness)?;
        self.inner.readiness(ids).await
    }
    async fn create(&self, draft: TaskDraft) -> TaskResult<Task> {
        self.gate.pass(Op::Create)?;
        self.inner.create(draft).await
    }
    async fn update(
        &self,
        id: ObjectId,
        expected: Revision,
        change: TaskPatch,
        scope: SeriesScope,
    ) -> TaskResult<Task> {
        self.gate.pass(Op::Update)?;
        self.inner.update(id, expected, change, scope).await
    }
    async fn assign(
        &self,
        id: ObjectId,
        expected: Revision,
        who: &PersonRef,
        scope: SeriesScope,
    ) -> TaskResult<Task> {
        self.gate.pass(Op::Assign)?;
        self.inner.assign(id, expected, who, scope).await
    }
    async fn unassign(
        &self,
        id: ObjectId,
        expected: Revision,
        who: &PersonRef,
        scope: SeriesScope,
    ) -> TaskResult<Task> {
        self.gate.pass(Op::Unassign)?;
        self.inner.unassign(id, expected, who, scope).await
    }
    async fn respond(
        &self,
        id: ObjectId,
        expected: Revision,
        answer: Answer,
        comment: Option<&str>,
        scope: SeriesScope,
    ) -> TaskResult<Task> {
        self.gate.pass(Op::Respond)?;
        self.inner
            .respond(id, expected, answer, comment, scope)
            .await
    }
    async fn progress(
        &self,
        id: ObjectId,
        expected: Revision,
        to: Progress,
        children: Children,
        scope: SeriesScope,
    ) -> TaskResult<Task> {
        self.gate.pass(Op::Progress)?;
        self.inner.progress(id, expected, to, children, scope).await
    }
    async fn place(
        &self,
        id: ObjectId,
        expected: Revision,
        after: Option<ObjectId>,
    ) -> TaskResult<Task> {
        self.gate.pass(Op::Place)?;
        self.inner.place(id, expected, after).await
    }
    async fn relate(
        &self,
        id: ObjectId,
        expected: Revision,
        dependency: Dependency,
    ) -> TaskResult<Task> {
        self.gate.pass(Op::Relate)?;
        self.inner.relate(id, expected, dependency).await
    }
    async fn unrelate(
        &self,
        id: ObjectId,
        expected: Revision,
        other_uid: &str,
        kind: DependencyKind,
    ) -> TaskResult<Task> {
        self.gate.pass(Op::Unrelate)?;
        self.inner.unrelate(id, expected, other_uid, kind).await
    }
    async fn claim(&self, id: ObjectId, expected: Revision, until: Utc) -> TaskResult<Task> {
        self.gate.pass(Op::Claim)?;
        self.inner.claim(id, expected, until).await
    }
    async fn release(&self, id: ObjectId, expected: Revision) -> TaskResult<Task> {
        self.gate.pass(Op::Release)?;
        self.inner.release(id, expected).await
    }
    async fn delete(
        &self,
        id: ObjectId,
        expected: Revision,
        scope: SeriesScope,
    ) -> TaskResult<Deleted> {
        self.gate.pass(Op::Delete)?;
        self.inner.delete(id, expected, scope).await
    }
    async fn restore(&self, id: ObjectId, scope: SeriesScope) -> TaskResult<Task> {
        self.gate.pass(Op::Restore)?;
        self.inner.restore(id, scope).await
    }
    async fn history(
        &self,
        id: ObjectId,
        after: Option<&Place>,
        size: usize,
    ) -> TaskResult<Page<HistoryEntry>> {
        self.gate.pass(Op::History)?;
        self.inner.history(id, after, size).await
    }
    async fn changes(&self, since: Option<&Place>, size: usize) -> TaskResult<Page<Change>> {
        self.gate.pass(Op::Changes)?;
        self.inner.changes(since, size).await
    }
}
