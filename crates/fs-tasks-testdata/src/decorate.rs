//! The decorator both doubles share: every method of `Tasks` goes through one gate, then to the provider inside.
//!
//! Pattern: Decorator (over `dyn Tasks`) with a Strategy for the gate (`FailGate`, `SlowGate`).

use crate::tasks_v1::{
    Answer, Capabilities, Change, ChecklistChange, Children, Deleted, Dependency, DependencyKind,
    HistoryEntry, LabelCount, ObjectId, Page, PersonRef, Place, Progress, Readiness, Revision,
    SeriesScope, Task, TaskDraft, TaskList, TaskListPatch, TaskPatch, TaskQuery, TaskResult,
    TaskSummary, Tasks, Trash, Utc,
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
    Checklist,
    Relate,
    Unrelate,
    Claim,
    Release,
    ClaimNext,
    Delete,
    Restore,
    Purge,
    History,
    Changes,
}

/// What a gate decides before a call: let it pass (maybe after a virtual wait) or answer with an error.
#[async_trait]
pub trait Gate: Send + Sync {
    /// Waits (`SlowGate` moves the fake's clock, `HoldGate` stays open until it is released) and then lets the call
    /// pass.
    ///
    /// # Errors
    /// The error the call answers with instead of reaching the provider inside.
    async fn pass(&self, op: Op) -> TaskResult<()>;

    /// Same for `list`, which knows the lists asked for (a partly broken provider, plan Z4); an empty slice means
    /// "all lists", so every broken list is among them.
    ///
    /// # Errors
    /// As [`Gate::pass`].
    async fn pass_list(&self, lists: &[ObjectId]) -> TaskResult<()> {
        let _ = lists;
        self.pass(Op::List).await
    }

    /// The capabilities shown (a staged provider switches some off; the default shows the inner ones).
    fn capabilities(&self, inner: Capabilities) -> Capabilities {
        inner
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
        self.gate.capabilities(self.inner.capabilities())
    }
    async fn asker(&self) -> TaskResult<PersonRef> {
        self.gate.pass(Op::Asker).await?;
        self.inner.asker().await
    }
    async fn lists(&self, trash: Trash) -> TaskResult<Vec<TaskList>> {
        self.gate.pass(Op::Lists).await?;
        self.inner.lists(trash).await
    }
    async fn create_list(
        &self,
        name: &str,
        color: Option<&str>,
        owner: Option<&PersonRef>,
    ) -> TaskResult<TaskList> {
        self.gate.pass(Op::CreateList).await?;
        self.inner.create_list(name, color, owner).await
    }
    async fn update_list(
        &self,
        id: ObjectId,
        expected: Revision,
        change: TaskListPatch,
    ) -> TaskResult<TaskList> {
        self.gate.pass(Op::UpdateList).await?;
        self.inner.update_list(id, expected, change).await
    }
    async fn delete_list(&self, id: ObjectId, expected: Revision) -> TaskResult<Deleted> {
        self.gate.pass(Op::DeleteList).await?;
        self.inner.delete_list(id, expected).await
    }
    async fn restore_list(&self, id: ObjectId) -> TaskResult<TaskList> {
        self.gate.pass(Op::RestoreList).await?;
        self.inner.restore_list(id).await
    }
    async fn get(&self, id: ObjectId) -> TaskResult<Option<Task>> {
        self.gate.pass(Op::Get).await?;
        self.inner.get(id).await
    }
    async fn get_by_uid(&self, uid: &str) -> TaskResult<Option<Task>> {
        self.gate.pass(Op::GetByUid).await?;
        self.inner.get_by_uid(uid).await
    }
    async fn list(
        &self,
        query: &TaskQuery,
        after: Option<&Place>,
        size: usize,
    ) -> TaskResult<Page<TaskSummary>> {
        self.gate.pass_list(&query.lists).await?;
        self.inner.list(query, after, size).await
    }
    async fn labels(
        &self,
        lists: &[ObjectId],
        prefix: Option<&str>,
        after: Option<&Place>,
        size: usize,
    ) -> TaskResult<Page<LabelCount>> {
        self.gate.pass(Op::Labels).await?;
        self.inner.labels(lists, prefix, after, size).await
    }
    async fn readiness(&self, ids: &[ObjectId]) -> TaskResult<Vec<(ObjectId, Readiness)>> {
        self.gate.pass(Op::Readiness).await?;
        self.inner.readiness(ids).await
    }
    async fn create(&self, draft: TaskDraft) -> TaskResult<Task> {
        self.gate.pass(Op::Create).await?;
        self.inner.create(draft).await
    }
    async fn update(
        &self,
        id: ObjectId,
        expected: Revision,
        change: TaskPatch,
        scope: SeriesScope,
    ) -> TaskResult<Task> {
        self.gate.pass(Op::Update).await?;
        self.inner.update(id, expected, change, scope).await
    }
    async fn assign(
        &self,
        id: ObjectId,
        expected: Revision,
        who: &PersonRef,
        scope: SeriesScope,
    ) -> TaskResult<Task> {
        self.gate.pass(Op::Assign).await?;
        self.inner.assign(id, expected, who, scope).await
    }
    async fn unassign(
        &self,
        id: ObjectId,
        expected: Revision,
        who: &PersonRef,
        scope: SeriesScope,
    ) -> TaskResult<Task> {
        self.gate.pass(Op::Unassign).await?;
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
        self.gate.pass(Op::Respond).await?;
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
        self.gate.pass(Op::Progress).await?;
        self.inner.progress(id, expected, to, children, scope).await
    }
    async fn place(
        &self,
        id: ObjectId,
        expected: Revision,
        after: Option<ObjectId>,
    ) -> TaskResult<Task> {
        self.gate.pass(Op::Place).await?;
        self.inner.place(id, expected, after).await
    }
    async fn checklist(
        &self,
        id: ObjectId,
        expected: Revision,
        change: ChecklistChange,
    ) -> TaskResult<Task> {
        self.gate.pass(Op::Checklist).await?;
        self.inner.checklist(id, expected, change).await
    }
    async fn relate(
        &self,
        id: ObjectId,
        expected: Revision,
        dependency: Dependency,
    ) -> TaskResult<Task> {
        self.gate.pass(Op::Relate).await?;
        self.inner.relate(id, expected, dependency).await
    }
    async fn unrelate(
        &self,
        id: ObjectId,
        expected: Revision,
        other_uid: &str,
        kind: DependencyKind,
    ) -> TaskResult<Task> {
        self.gate.pass(Op::Unrelate).await?;
        self.inner.unrelate(id, expected, other_uid, kind).await
    }
    async fn claim(&self, id: ObjectId, expected: Revision, until: Utc) -> TaskResult<Task> {
        self.gate.pass(Op::Claim).await?;
        self.inner.claim(id, expected, until).await
    }
    async fn release(&self, id: ObjectId, expected: Revision) -> TaskResult<Task> {
        self.gate.pass(Op::Release).await?;
        self.inner.release(id, expected).await
    }
    async fn claim_next(&self, query: &TaskQuery, until: Utc) -> TaskResult<Option<Task>> {
        self.gate.pass(Op::ClaimNext).await?;
        self.inner.claim_next(query, until).await
    }
    async fn delete(
        &self,
        id: ObjectId,
        expected: Revision,
        scope: SeriesScope,
    ) -> TaskResult<Deleted> {
        self.gate.pass(Op::Delete).await?;
        self.inner.delete(id, expected, scope).await
    }
    async fn restore(&self, id: ObjectId, scope: SeriesScope) -> TaskResult<Task> {
        self.gate.pass(Op::Restore).await?;
        self.inner.restore(id, scope).await
    }
    async fn purge(&self, id: ObjectId) -> TaskResult<()> {
        self.gate.pass(Op::Purge).await?;
        self.inner.purge(id).await
    }
    async fn history(
        &self,
        id: ObjectId,
        after: Option<&Place>,
        size: usize,
    ) -> TaskResult<Page<HistoryEntry>> {
        self.gate.pass(Op::History).await?;
        self.inner.history(id, after, size).await
    }
    async fn changes(&self, since: Option<&Place>, size: usize) -> TaskResult<Page<Change>> {
        self.gate.pass(Op::Changes).await?;
        self.inner.changes(since, size).await
    }
}
