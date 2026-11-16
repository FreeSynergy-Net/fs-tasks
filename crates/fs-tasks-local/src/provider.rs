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
use crate::unit::{read, write};
use crate::{
    checklist, deps, labels, lists, place, progress, query, reads, tasks_write, trash_ops,
};

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
        Ok(self.who().me)
    }

    async fn lists(&self, trash: Trash) -> TaskResult<Vec<TaskList>> {
        read!(self, w, c, lists::lists(&c, &w, trash))
    }

    async fn create_list(
        &self,
        name: &str,
        color: Option<&str>,
        owner: Option<&PersonRef>,
    ) -> TaskResult<TaskList> {
        write!(
            self,
            w,
            c,
            now,
            lists::create(&c, &w, now, name, color, owner)
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
            w,
            c,
            now,
            lists::update(&c, &w, now, id, expected, change)
        )
    }

    async fn delete_list(&self, id: ObjectId, expected: Revision) -> TaskResult<Deleted> {
        write!(self, w, c, now, lists::delete(&c, &w, now, id, expected))
    }

    async fn restore_list(&self, id: ObjectId) -> TaskResult<TaskList> {
        write!(self, w, c, now, lists::restore(&c, &w, now, id))
    }

    async fn get(&self, id: ObjectId) -> TaskResult<Option<Task>> {
        read!(self, w, c, reads::get(&c, &w, id))
    }

    async fn get_by_uid(&self, uid: &str) -> TaskResult<Option<Task>> {
        read!(self, w, c, reads::get_by_uid(&c, &w, uid))
    }

    async fn list(
        &self,
        q: &TaskQuery,
        after: Option<&Place>,
        size: usize,
    ) -> TaskResult<Page<TaskSummary>> {
        read!(self, w, c, query::list(&c, &w, q, after, size))
    }

    async fn labels(
        &self,
        lists: &[ObjectId],
        prefix: Option<&str>,
        after: Option<&Place>,
        size: usize,
    ) -> TaskResult<Page<LabelCount>> {
        read!(
            self,
            w,
            c,
            labels::labels(&c, &w, lists, prefix, after, size)
        )
    }

    async fn readiness(&self, ids: &[ObjectId]) -> TaskResult<Vec<(ObjectId, Readiness)>> {
        read!(self, w, c, deps::readiness(&c, &w, ids))
    }

    async fn create(&self, draft: TaskDraft) -> TaskResult<Task> {
        write!(self, w, c, now, tasks_write::create(&c, &w, now, draft))
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
            w,
            c,
            now,
            tasks_write::update(&c, &w, now, id, expected, change, scope)
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
        id: ObjectId,
        expected: Revision,
        to: Progress,
        children: Children,
        scope: SeriesScope,
    ) -> TaskResult<Task> {
        write!(
            self,
            w,
            c,
            now,
            progress::progress(&c, &w, now, id, expected, to, children, scope)
        )
    }

    async fn place(
        &self,
        id: ObjectId,
        expected: Revision,
        after: Option<ObjectId>,
    ) -> TaskResult<Task> {
        write!(
            self,
            w,
            c,
            now,
            place::place(&c, &w, now, id, expected, after)
        )
    }

    async fn checklist(
        &self,
        id: ObjectId,
        expected: Revision,
        change: ChecklistChange,
    ) -> TaskResult<Task> {
        write!(
            self,
            w,
            c,
            now,
            checklist::change(&c, &w, now, id, expected, change)
        )
    }

    async fn relate(
        &self,
        id: ObjectId,
        expected: Revision,
        dependency: Dependency,
    ) -> TaskResult<Task> {
        write!(
            self,
            w,
            c,
            now,
            deps::relate(&c, &w, now, id, expected, dependency)
        )
    }

    async fn unrelate(
        &self,
        id: ObjectId,
        expected: Revision,
        other_uid: &str,
        kind: DependencyKind,
    ) -> TaskResult<Task> {
        write!(
            self,
            w,
            c,
            now,
            deps::unrelate(&c, &w, now, id, expected, other_uid, kind)
        )
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
            w,
            c,
            now,
            trash_ops::delete(&c, &w, now, id, expected, scope)
        )
    }

    async fn restore(&self, id: ObjectId, scope: SeriesScope) -> TaskResult<Task> {
        write!(self, w, c, now, trash_ops::restore(&c, &w, now, id, scope))
    }

    async fn purge(&self, id: ObjectId) -> TaskResult<()> {
        write!(self, w, c, _now, trash_ops::purge(&c, &w, id))
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
