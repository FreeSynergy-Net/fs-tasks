//! `impl Tasks for FakeTasks`: every trait method locks the state once and calls the matching command or query of
//! the state (one call, one transaction; nothing is awaited while the lock is held).
//!
//! Pattern: Facade (thin trait methods over the state methods in `ops_*.rs` and `select.rs`).

use crate::fake::FakeTasks;
use crate::tasks_v1::{
    Answer, CAPABILITIES, Capabilities, Change, ChecklistChange, Children, Deleted, Dependency,
    DependencyKind, HistoryEntry, LabelCount, ObjectId, Page, PersonRef, Place, Progress,
    Readiness, Revision, SeriesScope, Task, TaskDraft, TaskList, TaskListPatch, TaskPatch,
    TaskQuery, TaskResult, TaskSummary, Tasks, Trash, Utc,
};
use async_trait::async_trait;

#[async_trait]
impl Tasks for FakeTasks {
    fn capabilities(&self) -> Capabilities {
        let evaluate = self.lock().evaluate;
        Capabilities(
            CAPABILITIES
                .iter()
                .map(|c| ((*c).to_owned(), evaluate || *c != "dependency.evaluate"))
                .collect(),
        )
    }

    async fn asker(&self) -> TaskResult<PersonRef> {
        Ok(self.lock().asker())
    }

    async fn lists(&self, trash: Trash) -> TaskResult<Vec<TaskList>> {
        let mut s = self.lock();
        s.expire();
        Ok(s.lists_of(trash))
    }

    async fn create_list(
        &self,
        name: &str,
        color: Option<&str>,
        owner: Option<&PersonRef>,
    ) -> TaskResult<TaskList> {
        self.lock().create_list(name, color, owner)
    }

    async fn update_list(
        &self,
        id: ObjectId,
        expected: Revision,
        change: TaskListPatch,
    ) -> TaskResult<TaskList> {
        self.lock().update_list(id, expected, change)
    }

    async fn delete_list(&self, id: ObjectId, expected: Revision) -> TaskResult<Deleted> {
        self.lock().delete_list(id, expected)
    }

    async fn restore_list(&self, id: ObjectId) -> TaskResult<TaskList> {
        self.lock().restore_list(id)
    }

    async fn get(&self, id: ObjectId) -> TaskResult<Option<Task>> {
        let mut s = self.lock();
        s.expire();
        Ok(s.live(id).ok().map(|i| s.rows[i].task.clone()))
    }

    async fn get_by_uid(&self, uid: &str) -> TaskResult<Option<Task>> {
        let mut s = self.lock();
        s.expire();
        let found = (0..s.rows.len())
            .find(|&i| s.rows[i].task.uid == uid && s.trash_of(i).is_none() && s.sees_task(i));
        Ok(found.map(|i| s.rows[i].task.clone()))
    }

    async fn list(
        &self,
        query: &TaskQuery,
        after: Option<&Place>,
        size: usize,
    ) -> TaskResult<Page<TaskSummary>> {
        let mut s = self.lock();
        s.expire();
        s.query(query, after, size)
    }

    async fn labels(
        &self,
        lists: &[ObjectId],
        prefix: Option<&str>,
        after: Option<&Place>,
        size: usize,
    ) -> TaskResult<Page<LabelCount>> {
        let mut s = self.lock();
        s.expire();
        s.label_counts(lists, prefix, after, size)
    }

    async fn readiness(&self, ids: &[ObjectId]) -> TaskResult<Vec<(ObjectId, Readiness)>> {
        let mut s = self.lock();
        s.expire();
        s.readiness_many(ids)
    }

    async fn create(&self, draft: TaskDraft) -> TaskResult<Task> {
        self.lock().create(draft)
    }

    async fn update(
        &self,
        id: ObjectId,
        expected: Revision,
        change: TaskPatch,
        scope: SeriesScope,
    ) -> TaskResult<Task> {
        self.lock().update(id, expected, change, scope)
    }

    async fn assign(
        &self,
        id: ObjectId,
        expected: Revision,
        who: &PersonRef,
        scope: SeriesScope,
    ) -> TaskResult<Task> {
        self.lock().assign(id, expected, who, scope)
    }

    async fn unassign(
        &self,
        id: ObjectId,
        expected: Revision,
        who: &PersonRef,
        scope: SeriesScope,
    ) -> TaskResult<Task> {
        self.lock().unassign(id, expected, who, scope)
    }

    async fn respond(
        &self,
        id: ObjectId,
        expected: Revision,
        answer: Answer,
        comment: Option<&str>,
        scope: SeriesScope,
    ) -> TaskResult<Task> {
        self.lock().respond(id, expected, answer, comment, scope)
    }

    async fn progress(
        &self,
        id: ObjectId,
        expected: Revision,
        to: Progress,
        children: Children,
        scope: SeriesScope,
    ) -> TaskResult<Task> {
        self.lock().progress(id, expected, to, children, scope)
    }

    async fn place(
        &self,
        id: ObjectId,
        expected: Revision,
        after: Option<ObjectId>,
    ) -> TaskResult<Task> {
        self.lock().place(id, expected, after)
    }

    async fn checklist(
        &self,
        id: ObjectId,
        expected: Revision,
        change: ChecklistChange,
    ) -> TaskResult<Task> {
        self.lock().checklist(id, expected, change)
    }

    async fn relate(
        &self,
        id: ObjectId,
        expected: Revision,
        dependency: Dependency,
    ) -> TaskResult<Task> {
        self.lock().relate(id, expected, dependency)
    }

    async fn unrelate(
        &self,
        id: ObjectId,
        expected: Revision,
        other_uid: &str,
        kind: DependencyKind,
    ) -> TaskResult<Task> {
        self.lock().unrelate(id, expected, other_uid, kind)
    }

    async fn claim(&self, id: ObjectId, expected: Revision, until: Utc) -> TaskResult<Task> {
        self.lock().claim(id, expected, until)
    }

    async fn release(&self, id: ObjectId, expected: Revision) -> TaskResult<Task> {
        self.lock().release(id, expected)
    }

    async fn claim_next(&self, query: &TaskQuery, until: Utc) -> TaskResult<Option<Task>> {
        self.lock().claim_next(query, until)
    }

    async fn delete(
        &self,
        id: ObjectId,
        expected: Revision,
        scope: SeriesScope,
    ) -> TaskResult<Deleted> {
        self.lock().delete(id, expected, scope)
    }

    async fn restore(&self, id: ObjectId, scope: SeriesScope) -> TaskResult<Task> {
        self.lock().restore(id, scope)
    }

    async fn purge(&self, id: ObjectId) -> TaskResult<()> {
        self.lock().purge_now(id)
    }

    async fn history(
        &self,
        id: ObjectId,
        after: Option<&Place>,
        size: usize,
    ) -> TaskResult<Page<HistoryEntry>> {
        let mut s = self.lock();
        s.expire();
        s.log_of(id, after, size)
    }

    async fn changes(&self, since: Option<&Place>, size: usize) -> TaskResult<Page<Change>> {
        let mut s = self.lock();
        s.expire();
        s.feed_since(since, size)
    }
}
