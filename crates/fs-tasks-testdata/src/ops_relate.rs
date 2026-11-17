//! Dependencies between tasks: `relate` (with the cycle check) and `unrelate`.
//!
//! Pattern: Command methods of the state. The cycle check runs in the transaction; a cycle that appears later (after
//! a sync) is not refused but shown as `Readiness::Conflict` (ADR 0073 point 8).

use crate::patch::cycle;
use crate::state::State;
use crate::tasks_v1::{
    Dependency, DependencyKind, HistoryKind, ObjectId, Revision, Task, TaskResult,
};

impl State {
    pub(crate) fn relate(
        &mut self,
        id: ObjectId,
        expected: Revision,
        dep: Dependency,
    ) -> TaskResult<Task> {
        self.expire();
        let i = self.live_expected(id, expected)?;
        self.need_write(self.rows[i].task.list, "relation.create")?;
        if dep.kind == DependencyKind::DependsOn && dep.gap.is_some() {
            return Err(Self::bad("gap", "not-with-depends-on"));
        }
        if dep.other_uid.is_empty() {
            return Err(Self::bad("other_uid", "empty"));
        }
        let uid = self.rows[i].task.uid.clone();
        if dep.other_uid == uid {
            return Err(cycle(vec![uid]));
        }
        if self.rows[i]
            .task
            .dependencies
            .iter()
            .any(|d| d.other_uid == dep.other_uid && d.kind == dep.kind)
        {
            return Ok(self.rows[i].task.clone());
        }
        if let Some(path) = self.find_cycle(&uid, Some((&uid, &dep.other_uid, dep.kind))) {
            return Err(cycle(path));
        }
        self.rows[i].task.dependencies.push(dep);
        self.bump(i, HistoryKind::Add, Vec::new());
        Ok(self.rows[i].task.clone())
    }

    pub(crate) fn unrelate(
        &mut self,
        id: ObjectId,
        expected: Revision,
        other_uid: &str,
        kind: DependencyKind,
    ) -> TaskResult<Task> {
        self.expire();
        let i = self.live_expected(id, expected)?;
        self.need_write(self.rows[i].task.list, "relation.delete")?;
        let deps = &mut self.rows[i].task.dependencies;
        let before = deps.len();
        deps.retain(|d| !(d.other_uid == other_uid && d.kind == kind));
        if deps.len() == before {
            return Err(Self::bad("other_uid", "not-related"));
        }
        self.bump(i, HistoryKind::Remove, Vec::new());
        Ok(self.rows[i].task.clone())
    }
}
