//! Lists: create, change (name, colour, owner), delete into the trash (with all their tasks) and restore.
//!
//! Pattern: Command methods of the state (each one a transaction; compare-and-set against `expected`).

use crate::state::{State, check};
use crate::tasks_v1::{
    Deleted, Error, HistoryKind, ListAccess, ListSource, ObjectId, PersonRef, Revision,
    SeriesScope, TaskList, TaskListPatch, TaskResult,
};

impl State {
    pub(crate) fn create_list(
        &mut self,
        name: &str,
        color: Option<&str>,
        owner: Option<&PersonRef>,
    ) -> TaskResult<TaskList> {
        self.expire();
        if name.trim().is_empty() {
            return Err(Self::bad("name", "empty"));
        }
        let owner = owner.cloned().unwrap_or_else(|| self.asker());
        self.check_owner(&owner, "list.create")?;
        let id = self.next_id();
        self.tick();
        let list = TaskList {
            id,
            name: name.to_owned(),
            color: color.map(str::to_owned),
            owner,
            access: ListAccess::Manage,
            source: ListSource::Own,
            revision: Revision(1),
            trash: None,
        };
        self.lists.push(list);
        Ok(self.shown(self.lists.len() - 1))
    }

    /// The list at `i` as the asking person sees it: their access and the trash state filled in.
    pub(crate) fn shown(&self, i: usize) -> TaskList {
        let mut l = self.lists[i].clone();
        l.access = self.access_of(&l).unwrap_or(ListAccess::Read);
        l.trash = self.list_trash(l.id);
        l
    }

    pub(crate) fn update_list(
        &mut self,
        id: ObjectId,
        expected: Revision,
        patch: TaskListPatch,
    ) -> TaskResult<TaskList> {
        self.expire();
        let i = self.live_list(id)?;
        self.need_manage(id, "list.edit")?;
        check(self.lists[i].revision, expected)?;
        if patch.name.as_ref().is_some_and(|n| n.trim().is_empty()) {
            return Err(Self::bad("name", "empty"));
        }
        if let Some(o) = &patch.owner {
            self.check_owner(o, "list.edit")?;
        }
        let before = self.lists[i].clone();
        let l = &mut self.lists[i];
        if let Some(n) = patch.name {
            l.name = n;
        }
        if let Some(c) = patch.color {
            l.color = c;
        }
        if let Some(o) = patch.owner {
            l.owner = o;
        }
        if *l != before {
            l.revision = Revision(l.revision.0 + 1);
            self.tick();
        }
        Ok(self.shown(i))
    }

    /// The list goes into the trash with ALL its living tasks (also subtasks), all with the same deadline.
    pub(crate) fn delete_list(&mut self, id: ObjectId, expected: Revision) -> TaskResult<Deleted> {
        self.expire();
        let i = self.live_list(id)?;
        self.need_manage(id, "list.delete")?;
        check(self.lists[i].revision, expected)?;
        self.tick();
        let bin = self.new_bin();
        let tasks: Vec<usize> = (0..self.rows.len())
            .filter(|&k| self.rows[k].task.list == id && self.trash_of(k).is_none())
            .collect();
        for &k in &tasks {
            let t = self.rows[k].task.id;
            self.feed(t, id, HistoryKind::Delete);
        }
        self.list_bins.insert(id, bin);
        self.lists[i].revision = Revision(self.lists[i].revision.0 + 1);
        Ok(Deleted {
            id,
            scope: SeriesScope::Whole,
            with_subtasks: tasks.into_iter().map(|k| self.rows[k].task.id).collect(),
            restorable: self.list_trash(id),
        })
    }

    /// The list and the tasks that lay in it when it was deleted come back; tasks deleted one by one stay deleted.
    pub(crate) fn restore_list(&mut self, id: ObjectId) -> TaskResult<TaskList> {
        self.expire();
        let i = self
            .list_at(id)
            .filter(|_| self.list_bins.contains_key(&id))
            .ok_or(Error::NotFound(id))?;
        self.need_manage(id, "list.delete")?;
        self.tick();
        self.list_bins.remove(&id);
        self.lists[i].revision = Revision(self.lists[i].revision.0 + 1);
        for k in 0..self.rows.len() {
            if self.rows[k].task.list == id && self.rows[k].deleted.is_none() {
                let t = self.rows[k].task.id;
                self.feed(t, id, HistoryKind::Undo);
            }
        }
        Ok(self.shown(i))
    }
}
