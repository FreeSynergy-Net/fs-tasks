//! Hand order: the rank of a task among its neighbours, with the one-time re-spread (`Rank::spread`).
//!
//! Pattern: Strategy (the rule of `rank.rs` in the locked folder: `between`, and `spread` on `Exhausted`/`Equal`,
//! in the same transaction, without `Changed` for the caller).

use crate::mutation::Mutation;
use crate::state::State;
use crate::tasks_v1::{HistoryKind, ObjectId, Progress, Rank, RankError, Task, TaskResult};

/// A rank sequence (list, parent, column) with the rows in it.
type Group = ((ObjectId, Option<ObjectId>, Progress), Vec<usize>);

impl State {
    /// The other living tasks of the rank sequence of `t` (same list, same parent, same column), by rank.
    pub(crate) fn column(&self, t: &Task) -> Vec<(Rank, ObjectId)> {
        let mut v: Vec<(Rank, ObjectId)> = self
            .rows
            .iter()
            .filter(|r| {
                r.deleted.is_none()
                    && r.task.id != t.id
                    && r.task.list == t.list
                    && r.task.parent == t.parent
                    && r.task.progress == t.progress
            })
            .map(|r| (r.task.rank.clone(), r.task.id))
            .collect();
        v.sort();
        v
    }

    /// The rank right after `after` (`None` = at the top) in the sequence of `t`. On `Exhausted`/`Equal` the
    /// sequence is spread once (revision +1 and a `Move` in the feed for each task whose rank changes), then the
    /// rank is computed again. Only swapped neighbours are `Invalid`.
    pub(crate) fn rank_after(&mut self, t: &Task, after: Option<ObjectId>) -> TaskResult<Rank> {
        for _ in 0..2 {
            let col = self.column(t);
            let (before, next) = match after {
                None => (None, col.first()),
                Some(a) => {
                    let i = col
                        .iter()
                        .position(|c| c.1 == a)
                        .ok_or_else(|| Self::bad("after", "not-in-sequence"))?;
                    (Some(&col[i].0), col.get(i + 1))
                }
            };
            match Rank::between(before, next.map(|n| &n.0)) {
                Ok(rank) => return Ok(rank),
                Err(RankError::Swapped) => return Err(Self::bad("rank", "swapped")),
                Err(RankError::Equal | RankError::Exhausted) => {
                    if after.is_none() && self.mutated(Mutation::TopNoSpread) {
                        return Err(Self::bad("rank", "exhausted"));
                    }
                    let ids: Vec<ObjectId> = col.iter().map(|c| c.1).collect();
                    for (id, rank) in ids.into_iter().zip(Rank::spread(col.len())) {
                        if let Some(i) = self.at(id) {
                            self.rows[i].task.rank = rank;
                            if self.mutated(Mutation::SpreadSilent) {
                                continue;
                            }
                            self.bump_quiet(i, HistoryKind::Move);
                        }
                    }
                }
            }
        }
        Err(Self::bad("rank", "exhausted"))
    }

    /// The rank at the top of the sequence of `t` (new and reopened cards go there).
    pub(crate) fn top(&mut self, t: &Task) -> TaskResult<Rank> {
        self.rank_after(t, None)
    }
}

impl State {
    /// Give every task a rank in its sequence, in insertion order (loading the sample).
    pub(crate) fn respread_all(&mut self) {
        let mut groups: Vec<Group> = Vec::new();
        for (i, r) in self.rows.iter().enumerate() {
            let key = (r.task.list, r.task.parent, r.task.progress);
            match groups.iter_mut().find(|g| g.0 == key) {
                Some(g) => g.1.push(i),
                None => groups.push((key, vec![i])),
            }
        }
        for (_, rows) in groups {
            for (i, rank) in rows.iter().zip(Rank::spread(rows.len())) {
                self.rows[*i].task.rank = rank;
            }
        }
    }
}
