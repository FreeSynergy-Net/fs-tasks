//! Lists: create, rename and recolour, delete into the trash (with all their tasks) and restore.
//!
//! Pattern: Command methods of the state (each one a transaction; compare-and-set against `expected`).

use crate::state::{State, check};
use crate::tasks_v1::{
    Deleted, Error, HistoryKind, ObjectId, Revision, SeriesScope, TaskList, TaskResult,
};

impl State {
    pub(crate) fn create_list(&mut self, name: &str, color: Option<&str>) -> TaskResult<TaskList> {
        self.purge();
        if name.trim().is_empty() {
            return Err(Self::bad("name", "empty"));
        }
        let id = self.next_id();
        self.tick();
        let list = TaskList {
            id,
            name: name.to_owned(),
            color: color.map(str::to_owned),
            owner: self.account.clone(),
            revision: Revision(1),
            restorable_until: None,
        };
        self.lists.push(list.clone());
        Ok(list)
    }

    #[allow(clippy::option_option)] // the trait's own signature: `Some(None)` removes the colour
    pub(crate) fn update_list(
        &mut self,
        id: ObjectId,
        expected: Revision,
        name: Option<&str>,
        color: Option<Option<&str>>,
    ) -> TaskResult<TaskList> {
        self.purge();
        let i = self.live_list(id)?;
        check(self.lists[i].revision, expected)?;
        if name.is_some_and(|n| n.trim().is_empty()) {
            return Err(Self::bad("name", "empty"));
        }
        let before = self.lists[i].clone();
        let l = &mut self.lists[i];
        if let Some(n) = name {
            n.clone_into(&mut l.name);
        }
        if let Some(c) = color {
            l.color = c.map(str::to_owned);
        }
        if *l != before {
            l.revision = Revision(l.revision.0 + 1);
            self.tick();
        }
        Ok(self.lists[i].clone())
    }

    /// The list goes into the trash with ALL its living tasks (also subtasks), all with the same deadline.
    pub(crate) fn delete_list(&mut self, id: ObjectId, expected: Revision) -> TaskResult<Deleted> {
        self.purge();
        let i = self.live_list(id)?;
        check(self.lists[i].revision, expected)?;
        self.tick();
        let until = self.trash_until();
        let tasks: Vec<usize> = (0..self.rows.len())
            .filter(|&k| self.rows[k].task.list == id && self.deleted_until(k).is_none())
            .collect();
        for &k in &tasks {
            let (t, l) = (self.rows[k].task.id, id);
            self.feed(t, l, HistoryKind::Delete);
        }
        self.lists[i].restorable_until = Some(until);
        self.lists[i].revision = Revision(self.lists[i].revision.0 + 1);
        Ok(Deleted {
            id,
            scope: SeriesScope::Whole,
            with_subtasks: tasks.into_iter().map(|k| self.rows[k].task.id).collect(),
            restorable_until: Some(until),
        })
    }

    /// The list and the tasks that lay in it when it was deleted come back; tasks deleted one by one stay deleted.
    pub(crate) fn restore_list(&mut self, id: ObjectId) -> TaskResult<TaskList> {
        self.purge();
        let i = self
            .list_at(id)
            .filter(|&k| self.lists[k].restorable_until.is_some());
        let i = i.ok_or(Error::NotFound(id))?;
        self.tick();
        self.lists[i].restorable_until = None;
        self.lists[i].revision = Revision(self.lists[i].revision.0 + 1);
        for k in 0..self.rows.len() {
            if self.rows[k].task.list == id && self.rows[k].deleted.is_none() {
                let t = self.rows[k].task.id;
                self.feed(t, id, HistoryKind::Undo);
            }
        }
        Ok(self.lists[i].clone())
    }
}
