//! The trash of the fake: the deadline of a deletion, the keep (`keep_until`), what `Restorable` says, expiry and the
//! endless `purge`.
//!
//! Pattern: State (a [`Bin`] is a deletion; whether it is `Until`, `Held` or `Manual` is computed from the clock and
//! the keeps at every question, never stored, so a clock that moves changes the answer by itself).

use crate::mutation::Mutation;
use crate::state::State;
use crate::tasks_v1::{Error, ObjectId, Restorable, TaskResult, Utc};

/// One deletion: when the automatic purge happens (`None` = never, the setting "nie automatisch löschen").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Bin {
    pub until: Option<Utc>,
}

/// What a bin means at `now`, with a keep (`Some(None)` = kept without an end, `Some(Some(t))` = kept until `t`).
/// A keep goes before the deadline and before "never": `Held` while it lasts, then `Until`/`Manual` again.
#[allow(clippy::option_option)] // `Some(None)` = kept without an end, the same shape as `keep_until`
pub(crate) fn restorable(bin: Bin, keep: Option<Option<Utc>>, now: Utc) -> Restorable {
    match keep {
        Some(None) => Restorable::Held(None),
        Some(Some(k)) if k > now => Restorable::Held(Some(match bin.until {
            Some(u) if u > k => u,
            _ => k,
        })),
        _ => bin.until.map_or(Restorable::Manual, Restorable::Until),
    }
}

impl State {
    /// A new bin with the deadline of the setting.
    pub(crate) fn new_bin(&self) -> Bin {
        let from = if self.mutated(Mutation::FixedDeadline) {
            crate::fake::START_MS
        } else {
            self.now.millis()
        };
        Bin {
            until: self
                .trash_days
                .filter(|_| !self.mutated(Mutation::NoDeadline))
                .map(|d| Utc::from_millis(from + i64::from(d) * 86_400_000)),
        }
    }

    /// What the bin of object `id` means now.
    pub(crate) fn bin_state(&self, id: ObjectId, bin: Bin) -> Restorable {
        restorable(bin, self.keeps.get(&id).copied(), self.now)
    }

    /// Whether the object `id` is kept now (a keep that has not run out).
    pub(crate) fn is_held(&self, id: ObjectId) -> bool {
        match self.keeps.get(&id) {
            Some(None) => true,
            Some(Some(k)) => *k > self.now,
            None => false,
        }
    }

    /// The trash state of a list (`None` = it lives). A list past its deadline that holds a kept task stays: `Held` with
    /// the latest keep inside (`None` = one of them is kept without an end).
    pub(crate) fn list_trash(&self, id: ObjectId) -> Option<Restorable> {
        let bin = *self.list_bins.get(&id)?;
        let own = self.bin_state(id, bin);
        if !matches!(own, Restorable::Until(u) if u <= self.now) {
            return Some(own);
        }
        let mut latest: Option<Option<Utc>> = None;
        for r in self.rows.iter().filter(|r| r.task.list == id) {
            if self.is_held(r.task.id) {
                let k = self.keeps.get(&r.task.id).copied().flatten();
                latest = Some(match (latest, k) {
                    (Some(None), _) | (_, None) => None,
                    (Some(Some(a)), Some(b)) => Some(a.max(b)),
                    (None, Some(b)) => Some(b),
                });
            }
        }
        Some(latest.map_or(own, Restorable::Held))
    }

    /// The trash state of the task at row `i`: its own deletion, else the one of its list (`None` = it lives).
    pub(crate) fn trash_of(&self, i: usize) -> Option<Restorable> {
        let row = &self.rows[i];
        match row.deleted {
            Some(bin) => Some(self.bin_state(row.task.id, bin)),
            None => self.list_trash(row.task.list),
        }
    }

    /// Remove the rows `ids` with their log, and let their remaining subtasks (deleted on their own, or kept) become
    /// top-level tasks (PLAN O18: nothing is deleted through a `parent` row).
    pub(crate) fn drop_rows(&mut self, ids: &[ObjectId]) {
        for r in &mut self.rows {
            if r.task.parent.is_some_and(|p| ids.contains(&p)) && !ids.contains(&r.task.id) {
                r.task.parent = None;
                r.deleted_with = None;
            }
        }
        self.rows.retain(|r| !ids.contains(&r.task.id));
        self.history.retain(|(t, _)| !ids.contains(t));
        self.cuts.retain(|c| !ids.contains(&c.id));
        self.keeps.retain(|k, _| !ids.contains(k));
    }

    fn drop_list(&mut self, id: ObjectId) {
        let stay = self.mutated(Mutation::OrphansStay);
        let ids: Vec<ObjectId> = self
            .rows
            .iter()
            .filter(|r| r.task.list == id && !(r.deleted.is_some() && stay))
            .map(|r| r.task.id)
            .collect();
        self.drop_rows(&ids);
        self.lists.retain(|l| l.id != id);
        self.list_bins.remove(&id);
        self.keeps.remove(&id);
    }

    /// Everything whose deadline has passed goes (a kept task stays, and so does the list that holds it).
    pub(crate) fn expire(&mut self) {
        let now = self.now;
        let dead_lists: Vec<ObjectId> = self
            .list_bins
            .keys()
            .copied()
            .filter(|&l| matches!(self.list_trash(l), Some(Restorable::Until(u)) if u <= now))
            .collect();
        for l in dead_lists {
            self.drop_list(l);
        }
        let dead: Vec<ObjectId> = self
            .rows
            .iter()
            .filter(|r| {
                r.deleted.is_some_and(
                    |b| matches!(self.bin_state(r.task.id, b), Restorable::Until(u) if u <= now),
                )
            })
            .map(|r| r.task.id)
            .collect();
        self.drop_rows(&dead);
        self.cuts.retain(|c| c.until > now);
    }

    /// `purge`: a task or list in the trash goes now, with its log and checklist.
    pub(crate) fn purge_now(&mut self, id: ObjectId) -> TaskResult<()> {
        self.expire();
        let forbidden = || Error::Forbidden {
            capability: "trash.purge".to_owned(),
        };
        if self.list_bins.contains_key(&id) {
            self.need_manage(id, "trash.purge")?;
            let kept = self
                .rows
                .iter()
                .any(|r| r.task.list == id && self.is_held(r.task.id));
            if (self.is_held(id) || kept) && !self.mutated(Mutation::PurgeHeld) {
                return Err(forbidden().into());
            }
            self.drop_list(id);
            return Ok(());
        }
        // NotFound (also for what the asker may not see), then Forbidden, then Invalid: nothing is given away.
        let i = self
            .at(id)
            .filter(|&i| self.sees_task(i) || self.mutated(Mutation::LeakTrash))
            .ok_or(Error::NotFound(id))?;
        if !self.mutated(Mutation::LeakTrash) {
            self.need_write(self.rows[i].task.list, "trash.purge")?;
        }
        if self.rows[i].deleted.is_none() {
            return Err(Self::bad("id", "not-in-trash"));
        }
        if self.rows[i].deleted_with.is_some() {
            return Err(Self::bad("id", "deleted-with-parent"));
        }
        if self.is_held(id) && !self.mutated(Mutation::PurgeHeld) {
            return Err(forbidden().into());
        }
        let mut ids = vec![id];
        ids.extend(self.rows_going_with(id));
        self.drop_rows(&ids);
        Ok(())
    }

    /// The subtasks that go with the purge of `id`: those deleted with it, but not a kept one (it stays in the trash
    /// on its own, as `expire` does it) and not the subtasks below a kept one (they stay with it).
    fn rows_going_with(&mut self, id: ObjectId) -> Vec<ObjectId> {
        let with: Vec<ObjectId> = self
            .rows
            .iter()
            .filter(|r| r.deleted_with == Some(id))
            .map(|r| r.task.id)
            .collect();
        if self.mutated(Mutation::PurgeTakesKept) {
            return with;
        }
        let mut stay: Vec<ObjectId> = Vec::new();
        for &k in &with {
            let mut up = Some(k);
            while let Some(c) = up.filter(|c| *c != id) {
                if self.is_held(c) {
                    stay.push(k);
                    if c != k {
                        let to = c;
                        self.rows
                            .iter_mut()
                            .filter(|r| r.task.id == k)
                            .for_each(|r| {
                                r.deleted_with = Some(to);
                            });
                    }
                    break;
                }
                up = self.at(c).and_then(|i| self.rows[i].task.parent);
            }
        }
        with.into_iter().filter(|k| !stay.contains(k)).collect()
    }
}
