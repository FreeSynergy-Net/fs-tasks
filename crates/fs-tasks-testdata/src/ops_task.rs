//! Tasks: create and update (the fields), with the rules of the trait (`Invalid`, `Cycle`, series scope).
//!
//! Pattern: Command methods of the state (check everything first, then write; one transaction per call).

use crate::blank::blank_task;
use crate::ids::object_number;
use crate::mutation::Mutation;
use crate::ops_checklist::items_of;
use crate::patch::{apply, check_patch, check_task, cycle, diff};
use crate::ready::is_done;
use crate::scope::{Target, changed};
use crate::state::{Row, State};
use crate::tasks_v1::{
    Assignee, AssigneeStatus, HistoryKind, ObjectId, Revision, SUBTASK_DEPTH_MAX, SeriesScope,
    Task, TaskDraft, TaskPatch, TaskResult,
};

impl State {
    pub(crate) fn create(&mut self, d: TaskDraft) -> TaskResult<Task> {
        self.expire();
        if d.rest.title.is_some() || d.rest.list.is_some() {
            return Err(Self::bad("rest", "title-or-list-set-twice"));
        }
        if d.title.trim().is_empty() {
            return Err(Self::bad("title", "empty"));
        }
        self.live_list(d.list)?;
        self.need_write(d.list, "task.create")?;
        check_patch(&d.rest)?;
        if let Some(Some(p)) = d.rest.parent {
            let k = self.live(p)?;
            if self.rows[k].task.list != d.list {
                return Err(Self::bad("parent", "other-list"));
            }
            if self.depth_of(p) + 1 > SUBTASK_DEPTH_MAX && !self.mutated(Mutation::NoDepthLimit) {
                return Err(Self::bad("parent", "too-deep"));
            }
        }
        let mut keys = Vec::new();
        for _ in 0..d.checklist.len() {
            keys.push(self.next_key());
        }
        let mut keys = keys.into_iter();
        let items = items_of(&d.checklist, &mut || keys.next().unwrap_or_default())?;
        if d.uid
            .as_deref()
            .is_some_and(|u| u.is_empty() || self.rows.iter().any(|r| r.task.uid == u))
        {
            return Err(Self::bad("uid", "exists-or-empty"));
        }
        let id = self.next_id();
        let uid = d
            .uid
            .unwrap_or_else(|| format!("urn:fs-testdata:task-{}", object_number(id)));
        let now = self.tick();
        let mut t = blank_task(id, d.list, &uid, &d.title, now);
        t.progress = d.progress;
        t.parent = d.rest.parent.flatten();
        t.checklist = items;
        apply(&mut t, d.rest);
        check_task(&t)?;
        if is_done(t.progress) {
            t.progress_updated = Some(now);
        }
        let by = self.asker();
        for who in d.assignees {
            if t.assignees.iter().all(|a| a.who != who) {
                t.assignees.push(Assignee {
                    who,
                    status: AssigneeStatus::NeedsAction,
                    assigned_by: Some(by.clone()),
                    assigned: Some(now),
                    comment: None,
                });
            }
        }
        t.rank = self.top(&t)?;
        self.rows.push(Row {
            task: t.clone(),
            deleted: None,
            deleted_with: None,
        });
        self.note(self.rows.len() - 1, HistoryKind::Create, Vec::new());
        Ok(t)
    }

    pub(crate) fn update(
        &mut self,
        id: ObjectId,
        expected: Revision,
        patch: TaskPatch,
        scope: SeriesScope,
    ) -> TaskResult<Task> {
        self.expire();
        let i = self.live_expected(id, expected)?;
        self.need_write(self.rows[i].task.list, "task.edit")?;
        check_patch(&patch)?;
        let moves = patch.list.is_some() || patch.parent.is_some();
        let series = patch.recurrence.is_some()
            || patch.excluded_recurrence.is_some()
            || patch.repeat_from.is_some();
        if scope != SeriesScope::Whole && (moves || series) {
            return Err(Self::bad("patch", "whole-only"));
        }
        self.atomic(|s| match s.target(i, scope, "task.edit")? {
            Target::Whole(j) => s.update_whole(j, patch),
            Target::Occurrence(j, rid) => {
                let t = &mut s.rows[j].task;
                let (stored, _, _) = changed(t, &rid)?;
                merge(stored, patch);
                s.bump(j, HistoryKind::Update, Vec::new());
                Ok(s.rows[j].task.clone())
            }
        })
    }

    fn update_whole(&mut self, j: usize, patch: TaskPatch) -> TaskResult<Task> {
        let old = self.rows[j].task.clone();
        let list = patch.list.unwrap_or(old.list);
        let parent = patch.parent.unwrap_or(old.parent);
        if list != old.list {
            self.live_list(list)?;
        }
        if let Some(p) = parent {
            let k = self.live(p)?;
            if p == old.id || self.rows[k].task.list != list {
                return Err(if p == old.id {
                    cycle(vec![old.uid.clone()])
                } else {
                    Self::bad("parent", "other-list")
                });
            }
            if let Some(path) = self.parent_cycle(&old, p) {
                return Err(cycle(path));
            }
            if self.depth_of(p) + self.height_of(old.id) > SUBTASK_DEPTH_MAX {
                return Err(Self::bad("parent", "too-deep"));
            }
        }
        let mut t = old.clone();
        apply(&mut t, patch);
        check_task(&t)?;
        let moved = list != old.list || parent != old.parent;
        if moved {
            (t.list, t.parent) = (list, parent);
            t.rank = self.rank_after(&t, None)?;
        }
        if t == old {
            return Ok(old);
        }
        if list != old.list {
            for k in self.descendants(old.id) {
                self.rows[k].task.list = list;
                self.bump_quiet(k, HistoryKind::Move);
            }
        }
        let fields = diff(&old, &t);
        self.rows[j].task = t;
        self.bump(
            j,
            if moved {
                HistoryKind::Move
            } else {
                HistoryKind::Update
            },
            fields,
        );
        Ok(self.rows[j].task.clone())
    }

    /// The level of a task in its tree (a top-level task is level 1).
    pub(crate) fn depth_of(&self, id: ObjectId) -> usize {
        let mut n = 0;
        let mut at = Some(id);
        while let Some(i) = at {
            n += 1;
            at = self.at(i).and_then(|k| self.rows[k].task.parent);
            if n > 1000 {
                break;
            }
        }
        n
    }

    /// The levels of the tree below a task, the task itself counted (a task without subtasks is 1).
    fn height_of(&self, id: ObjectId) -> usize {
        1 + self
            .children(id)
            .into_iter()
            .map(|k| self.height_of(self.rows[k].task.id))
            .max()
            .unwrap_or(0)
    }

    /// The `uid`s along the way from `parent` up to `task` if hanging `task` under `parent` closes a cycle.
    fn parent_cycle(&self, task: &Task, parent: ObjectId) -> Option<Vec<String>> {
        let mut path = vec![task.uid.clone()];
        let mut at = Some(parent);
        while let Some(id) = at {
            let k = self.at(id)?;
            path.push(self.rows[k].task.uid.clone());
            if id == task.id {
                path.pop();
                return Some(path);
            }
            at = self.rows[k].task.parent;
        }
        None
    }
}

/// Fields of a patch named in `new` replace those of the stored override patch.
fn merge(stored: &mut TaskPatch, new: TaskPatch) {
    macro_rules! take {
        ($($f:ident),*) => {$( if new.$f.is_some() { stored.$f = new.$f; } )*};
    }
    take!(
        title,
        description,
        locale,
        due,
        start,
        estimated,
        percent_complete,
        priority,
        color,
        labels,
        alerts,
        links
    );
}
