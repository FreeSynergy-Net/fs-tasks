//! The tree of subtasks of one list (PLAN O18): one parent at most, never a cycle, at most `SUBTASK_DEPTH_MAX` levels.
//!
//! Pattern: Value Object built from one `SELECT` (the whole tree of a list in memory; every check is a pure function of
//! it, so the checks run before anything is written).

use std::collections::BTreeMap;

use fs_objects::v5;
use fs_objects::v5::codec::id_value;
use fs_tasks_testdata::tasks_v1::{ObjectId, TaskResult};

use crate::cur::Cur;
use crate::error::adapt;

/// One task in the tree.
#[derive(Clone, Debug)]
pub struct Node {
    pub parent: Option<ObjectId>,
    pub uid: String,
    /// Not deleted on its own, not with a parent (the list's trash state is the caller's question).
    pub alive: bool,
    /// The task it was deleted with.
    pub with: Option<ObjectId>,
}

/// All tasks of one list.
#[derive(Clone, Debug, Default)]
pub struct Tree(BTreeMap<ObjectId, Node>);

impl Tree {
    /// Read the tasks of `list`.
    pub async fn load(c: &v5::Conn<'_>, list: ObjectId) -> TaskResult<Self> {
        let sql = "SELECT t.id, t.parent, t.uid, t.deleted_with, o.deleted IS NULL \
                   FROM tasks_task t JOIN objects o ON o.id = t.id WHERE t.list = ?";
        let rows = c.query(sql, vec![id_value(list)]).await.map_err(adapt)?;
        let mut nodes = BTreeMap::new();
        for row in &rows {
            let mut cur = Cur::new(row);
            let id = cur.id()?;
            let parent = cur.opt_id()?;
            let uid = cur.text()?;
            let with = cur.opt_id()?;
            let own_alive = cur.flag()?;
            nodes.insert(
                id,
                Node {
                    parent,
                    uid,
                    alive: own_alive && with.is_none(),
                    with,
                },
            );
        }
        Ok(Self(nodes))
    }

    /// The level of a task (a top-level task is level 1).
    #[must_use]
    pub fn depth_of(&self, id: ObjectId) -> usize {
        let mut n = 0;
        let mut at = Some(id);
        while let Some(i) = at {
            n += 1;
            at = self.0.get(&i).and_then(|k| k.parent);
            if n > 1000 {
                break;
            }
        }
        n
    }

    /// The living subtasks, direct ones first level by level (sorted by id within a level).
    #[must_use]
    pub fn descendants(&self, id: ObjectId) -> Vec<ObjectId> {
        let mut out = Vec::new();
        let mut level = vec![id];
        while !level.is_empty() {
            let mut next: Vec<ObjectId> = self
                .0
                .iter()
                .filter(|(k, n)| {
                    n.alive && n.parent.is_some_and(|p| level.contains(&p)) && !out.contains(*k)
                })
                .map(|(k, _)| *k)
                .collect();
            next.sort();
            out.extend(next.iter().copied());
            level = next;
        }
        out
    }

    /// The levels of the tree below a task, the task itself counted (a task without living subtasks is 1).
    #[must_use]
    pub fn height_of(&self, id: ObjectId) -> usize {
        1 + self
            .0
            .iter()
            .filter(|(_, n)| n.alive && n.parent == Some(id))
            .map(|(k, _)| self.height_of(*k))
            .max()
            .unwrap_or(0)
    }

    /// The levels below a task of the subtasks that were deleted WITH it (they come back with it).
    #[must_use]
    pub fn below_with(&self, id: ObjectId) -> usize {
        let base = self.depth_of(id);
        self.0
            .iter()
            .filter(|(_, n)| n.with == Some(id))
            .map(|(k, _)| self.depth_of(*k).saturating_sub(base))
            .max()
            .unwrap_or(0)
    }

    /// The `uid`s along the way from `parent` up to `task`, if hanging `task` under `parent` closes a cycle.
    #[must_use]
    pub fn cycle_path(&self, task: ObjectId, parent: ObjectId) -> Option<Vec<String>> {
        let mut path = vec![self.0.get(&task)?.uid.clone()];
        let mut at = Some(parent);
        let mut guard = 0;
        while let Some(id) = at {
            let node = self.0.get(&id)?;
            path.push(node.uid.clone());
            if id == task {
                path.pop();
                return Some(path);
            }
            at = node.parent;
            guard += 1;
            if guard > 1000 {
                return None;
            }
        }
        None
    }

    /// Whether the task exists in the list.
    #[must_use]
    pub fn has(&self, id: ObjectId) -> bool {
        self.0.contains_key(&id)
    }

    /// The node of a task.
    #[must_use]
    pub fn node(&self, id: ObjectId) -> Option<&Node> {
        self.0.get(&id)
    }
}
