//! `LocalTasks`: the trait impl; one short body per method, the work is in `lists`, `tasks_write`, `trash_ops`,
//! `query`. What this run does not offer says so in `capabilities` and answers `Unsupported` (`not_possible`).
//!
//! Pattern: Adapter (trait to the typed store). Every call first runs the deadline (what the trash period ended goes
//! for good), then one unit of work: a read, or `begin` .. `finish`.

use async_trait::async_trait;
use fs_tasks_testdata::tasks_v1::{
    Answer, CAPABILITIES, Capabilities, Change, ChecklistChange, Children, Deleted, Dependency,
    DependencyKind, HistoryEntry, LabelCount, ObjectId, Page, PersonRef, Place, Progress,
    Readiness, Revision, SeriesScope, Task, TaskDraft, TaskList, TaskListPatch, TaskPatch,
    TaskQuery, TaskResult, TaskSummary, Tasks, Trash, Utc, not_possible,
};

use crate::local::{LocalTasks, OFFERED};
use crate::store::finish;
use crate::stored::load_one;
use crate::{lists, query, tasks_write, trash_ops};

/// One unit of work: the deadline run, the time of the write, `begin`, the body, `finish`.
macro_rules! write {
    ($s:expr, $c:ident, $now:ident, $body:expr) => {{
        $s.sweep().await?;
        let $now = $s.clock.tick();
        let $c = $s.store.begin().await?;
        let r = $body.await;
        finish($c, r).await
    }};
}

/// One read after the deadline run.
macro_rules! read {
    ($s:expr, $c:ident, $body:expr) => {{
        $s.sweep().await?;
        let $c = $s.store.read().await;
        $body.await
    }};
}

#[async_trait]
impl Tasks for LocalTasks {
    fn capabilities(&self) -> Capabilities {
        Capabilities(
            CAPABILITIES
                .iter()
                .map(|c| ((*c).to_owned(), OFFERED.contains(c)))
                .collect(),
        )
    }

    async fn asker(&self) -> TaskResult<PersonRef> {
        Ok(self.actor.clone())
    }

    async fn lists(&self, trash: Trash) -> TaskResult<Vec<TaskList>> {
        read!(self, c, lists::lists(&c, trash))
    }

    async fn create_list(
        &self,
        name: &str,
        color: Option<&str>,
        owner: Option<&PersonRef>,
    ) -> TaskResult<TaskList> {
        write!(
            self,
            c,
            now,
            lists::create(&c, &self.actor, now, name, color, owner)
        )
    }

    async fn update_list(
        &self,
        id: ObjectId,
        expected: Revision,
        change: TaskListPatch,
    ) -> TaskResult<TaskList> {
        write!(
            self,
            c,
            now,
            lists::update(&c, &self.actor, now, id, expected, change)
        )
    }

    async fn delete_list(&self, id: ObjectId, expected: Revision) -> TaskResult<Deleted> {
        write!(self, c, now, lists::delete(&c, now, id, expected))
    }

    async fn restore_list(&self, id: ObjectId) -> TaskResult<TaskList> {
        write!(self, c, now, lists::restore(&c, now, id))
    }

    async fn get(&self, id: ObjectId) -> TaskResult<Option<Task>> {
        read!(self, c, async {
            Ok(load_one(&c, id)
                .await?
                .filter(crate::stored::Stored::alive)
                .map(|s| s.task))
        })
    }

    async fn get_by_uid(&self, uid: &str) -> TaskResult<Option<Task>> {
        read!(self, c, async {
            let found =
                crate::stored::load(&c, "t.uid = ?", vec![fs_objects::v5::codec::text(uid)])
                    .await?;
            Ok(found
                .into_iter()
                .find(crate::stored::Stored::alive)
                .map(|s| s.task))
        })
    }

    async fn list(
        &self,
        q: &TaskQuery,
        after: Option<&Place>,
        size: usize,
    ) -> TaskResult<Page<TaskSummary>> {
        read!(self, c, query::list(&c, q, after, size))
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

    async fn create(&self, draft: TaskDraft) -> TaskResult<Task> {
        write!(
            self,
            c,
            now,
            tasks_write::create(&c, &self.actor, now, draft)
        )
    }

    async fn update(
        &self,
        id: ObjectId,
        expected: Revision,
        change: TaskPatch,
        scope: SeriesScope,
    ) -> TaskResult<Task> {
        write!(
            self,
            c,
            now,
            tasks_write::update(&c, now, id, expected, change, scope)
        )
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
        id: ObjectId,
        expected: Revision,
        scope: SeriesScope,
    ) -> TaskResult<Deleted> {
        write!(
            self,
            c,
            now,
            trash_ops::delete(&c, now, id, expected, scope)
        )
    }

    async fn restore(&self, id: ObjectId, scope: SeriesScope) -> TaskResult<Task> {
        write!(self, c, now, trash_ops::restore(&c, now, id, scope))
    }

    async fn purge(&self, id: ObjectId) -> TaskResult<()> {
        write!(self, c, _now, trash_ops::purge(&c, id))
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
        not_possible("task.changes")
    }
}
