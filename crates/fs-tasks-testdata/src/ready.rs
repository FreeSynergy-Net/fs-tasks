//! Dependencies: what a task waits for (RFC 9253), readiness, and cycle search.
//!
//! Pattern: Strategy (readiness with `dependency.evaluate`, or the honest `NotEvaluated` without it). The graph is
//! "waits for": a task holding `DependsOn` waits for the other; a task holding one of the four time kinds is the
//! predecessor, so the OTHER task waits for it. Parents and subtasks are a separate tree (cycles there are
//! `TaskError::Cycle` too, see `update`).

use crate::state::State;
use crate::tasks_v1::{DependencyKind, Progress, Readiness, Task};
use std::collections::BTreeSet;

/// Finished in any way: completed, cancelled or failed.
pub(crate) fn is_done(p: Progress) -> bool {
    matches!(
        p,
        Progress::Completed | Progress::Failed | Progress::Cancelled
    )
}

impl State {
    fn by_uid(&self, uid: &str) -> Option<&Task> {
        self.rows
            .iter()
            .enumerate()
            .find(|(i, r)| r.task.uid == uid && self.trash_of(*i).is_none())
            .map(|(_, r)| &r.task)
    }

    /// What `uid` waits for, with the kind of the tie; `extra` is an edge not stored yet (`relate`).
    fn waits_on(
        &self,
        uid: &str,
        extra: Option<(&str, &str, DependencyKind)>,
    ) -> Vec<(String, DependencyKind)> {
        let mut out = Vec::new();
        for (i, r) in self.rows.iter().enumerate() {
            if self.trash_of(i).is_some() {
                continue;
            }
            for d in &r.task.dependencies {
                let own = r.task.uid == uid;
                match d.kind {
                    DependencyKind::DependsOn if own => out.push((d.other_uid.clone(), d.kind)),
                    k if k != DependencyKind::DependsOn && d.other_uid == uid => {
                        out.push((r.task.uid.clone(), k));
                    }
                    _ => {}
                }
            }
        }
        if let Some((holder, other, kind)) = extra {
            if kind == DependencyKind::DependsOn && holder == uid {
                out.push((other.to_owned(), kind));
            } else if kind != DependencyKind::DependsOn && other == uid {
                out.push((holder.to_owned(), kind));
            }
        }
        out
    }

    /// The `uid`s along a cycle through `start` (with `extra` added), if there is one.
    pub(crate) fn find_cycle(
        &self,
        start: &str,
        extra: Option<(&str, &str, DependencyKind)>,
    ) -> Option<Vec<String>> {
        let mut seen = BTreeSet::new();
        let mut path = vec![start.to_owned()];
        self.walk(start, start, extra, &mut seen, &mut path)
            .then_some(path)
    }

    fn walk(
        &self,
        at: &str,
        start: &str,
        extra: Option<(&str, &str, DependencyKind)>,
        seen: &mut BTreeSet<String>,
        path: &mut Vec<String>,
    ) -> bool {
        for (next, _) in self.waits_on(at, extra) {
            if next == start {
                return true;
            }
            if seen.insert(next.clone()) {
                path.push(next.clone());
                if self.walk(&next, start, extra, seen, path) {
                    return true;
                }
                path.pop();
            }
        }
        false
    }

    /// The readiness of the task at row `i`.
    pub(crate) fn readiness_of(&self, i: usize) -> Readiness {
        let t = &self.rows[i].task;
        if is_done(t.progress) {
            return Readiness::Done;
        }
        if !self.evaluate {
            return if t.dependencies.is_empty() {
                Readiness::Ready
            } else {
                Readiness::NotEvaluated
            };
        }
        if let Some(mut cycle) = self.find_cycle(&t.uid, None) {
            cycle.sort();
            return Readiness::Conflict(cycle);
        }
        let unknown: Vec<String> = t
            .dependencies
            .iter()
            .filter(|d| self.by_uid(&d.other_uid).is_none())
            .map(|d| d.other_uid.clone())
            .collect();
        if !unknown.is_empty() {
            return Readiness::Unknown(unknown);
        }
        let (mut cancelled, mut blocked) = (Vec::new(), Vec::new());
        for (uid, kind) in self.waits_on(&t.uid, None) {
            let Some(p) = self.by_uid(&uid) else { continue };
            let started_enough = matches!(
                kind,
                DependencyKind::StartToStart | DependencyKind::StartToFinish
            ) && p.progress != Progress::NeedsAction;
            match p.progress {
                Progress::Cancelled | Progress::Failed => cancelled.push(uid),
                Progress::Completed => {}
                _ if started_enough => {}
                _ => blocked.push(uid),
            }
        }
        if !cancelled.is_empty() {
            Readiness::DependencyCancelled(cancelled)
        } else if !blocked.is_empty() {
            Readiness::Blocked(blocked)
        } else {
            Readiness::Ready
        }
    }
}
