//! The graph of dependencies "who waits for whom" (RFC 9253) and what it says about a task: a cycle, `Readiness`.
//!
//! Pattern: Strategy (readiness with `dependency.evaluate`) over a Value Object (the edges of the LIVING tasks, read
//! with one `SELECT`). A task holding `DependsOn` waits for the other; a task holding one of the four time kinds is the
//! predecessor, so the OTHER task waits for it. Parents and subtasks are a separate tree (`tree.rs`). The ready filter
//! of `list` is the same judgement in SQL (`ready_sql.rs`); the tests hold the two against each other.

use std::collections::{BTreeMap, BTreeSet};

use fs_objects::v5;
use fs_objects::v5::codec::text;
use fs_tasks_testdata::tasks_v1::{DependencyKind, Progress, Readiness, TaskResult};

use crate::cur::Cur;
use crate::error::adapt;
use crate::query::ALIVE;
use crate::stored::marks;
use crate::words::{is_done, kind_of, progress_of};

/// One tie as the holder wrote it: the holder's `uid`, the other `uid`, the kind.
type Edge = (String, String, DependencyKind);

/// A tie that is not stored yet (`relate`): holder, other, kind.
pub type Extra<'a> = Option<(&'a str, &'a str, DependencyKind)>;

/// The edges of the living tasks, and the progress of the living tasks they name.
#[derive(Debug, Default)]
pub struct Graph {
    edges: Vec<Edge>,
    live: BTreeMap<String, Progress>,
}

impl Graph {
    /// Read the edges (and, with `with_live`, the progress of every living task they name). Without any dependency
    /// row the graph is empty and no second `SELECT` runs.
    pub async fn load(c: &v5::Conn<'_>, with_live: bool) -> TaskResult<Self> {
        let sql = format!(
            "SELECT t.uid, d.other_uid, d.kind FROM tasks_dependency d JOIN tasks_task t ON t.id = d.task \
             JOIN objects o ON o.id = t.id JOIN objects lo ON lo.id = t.list \
             LEFT JOIN objects wo ON wo.id = t.deleted_with WHERE {ALIVE} ORDER BY t.id, d.pos"
        );
        let mut edges = Vec::new();
        for row in &c.query(&sql, Vec::new()).await.map_err(adapt)? {
            let mut cur = Cur::new(row);
            edges.push((cur.text()?, cur.text()?, kind_of(&cur.text()?)?));
        }
        let mut live = BTreeMap::new();
        if with_live && !edges.is_empty() {
            let uids: BTreeSet<&String> = edges.iter().flat_map(|e| [&e.0, &e.1]).collect();
            let uids: Vec<&String> = uids.into_iter().collect();
            for chunk in uids.chunks(400) {
                let sql = format!(
                    "SELECT t.uid, t.progress FROM tasks_task t JOIN objects o ON o.id = t.id \
                     JOIN objects lo ON lo.id = t.list LEFT JOIN objects wo ON wo.id = t.deleted_with \
                     WHERE {ALIVE} AND t.uid IN ({})",
                    marks(chunk.len())
                );
                let rows = c
                    .query(&sql, chunk.iter().map(|u| text(u)).collect())
                    .await
                    .map_err(adapt)?;
                for row in &rows {
                    let mut cur = Cur::new(row);
                    live.insert(cur.text()?, progress_of(&cur.text()?)?);
                }
            }
        }
        Ok(Self { edges, live })
    }

    /// Whether no living task holds a dependency.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }

    /// What `uid` waits for, with the kind of the tie, in the order the ties were written; `extra` is a tie not stored yet.
    fn waits_on(&self, uid: &str, extra: Extra<'_>) -> Vec<(String, DependencyKind)> {
        let mut out = Vec::new();
        for (holder, other, kind) in &self.edges {
            if *kind == DependencyKind::DependsOn && holder == uid {
                out.push((other.clone(), *kind));
            } else if *kind != DependencyKind::DependsOn && other == uid {
                out.push((holder.clone(), *kind));
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

    /// The `uid`s along a cycle through `start` (with `extra` added), if there is one: a depth-first walk along "waits
    /// for", the path from `start` to the tie that leads back to it.
    #[must_use]
    pub fn find_cycle(&self, start: &str, extra: Extra<'_>) -> Option<Vec<String>> {
        let next = |at: &str| -> Vec<String> {
            self.waits_on(at, extra).into_iter().map(|w| w.0).collect()
        };
        let mut seen = BTreeSet::new();
        let mut path = vec![start.to_owned()];
        let mut stack = vec![(next(start), 0_usize)];
        while let Some((ways, i)) = stack.last_mut() {
            let Some(to) = ways.get(*i).cloned() else {
                stack.pop();
                path.pop();
                continue;
            };
            *i += 1;
            if to == start {
                return Some(path);
            }
            if seen.insert(to.clone()) {
                path.push(to.clone());
                stack.push((next(&to), 0));
            }
        }
        None
    }

    /// The readiness of the task `uid` in `progress` (the order of the judgement: finished, cycle, unknown name, the
    /// predecessors).
    #[must_use]
    pub fn readiness(&self, uid: &str, progress: Progress) -> Readiness {
        if is_done(progress) {
            return Readiness::Done;
        }
        if self.edges.is_empty() {
            return Readiness::Ready;
        }
        if let Some(mut cycle) = self.find_cycle(uid, None) {
            cycle.sort();
            return Readiness::Conflict(cycle);
        }
        let unknown: Vec<String> = self
            .edges
            .iter()
            .filter(|e| e.0 == uid && !self.live.contains_key(&e.1))
            .map(|e| e.1.clone())
            .collect();
        if !unknown.is_empty() {
            return Readiness::Unknown(unknown);
        }
        let (mut cancelled, mut blocked) = (Vec::new(), Vec::new());
        for (pred, kind) in self.waits_on(uid, None) {
            let Some(p) = self.live.get(&pred) else {
                continue;
            };
            let started_enough = matches!(
                kind,
                DependencyKind::StartToStart | DependencyKind::StartToFinish
            ) && *p != Progress::NeedsAction;
            match p {
                Progress::Cancelled | Progress::Failed => cancelled.push(pred),
                Progress::Completed => {}
                _ if started_enough => {}
                _ => blocked.push(pred),
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
