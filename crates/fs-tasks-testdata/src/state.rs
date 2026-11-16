//! The in-memory state behind [`crate::FakeTasks`]: lists, tasks, the change feed, the log and the clock.
//!
//! Pattern: Repository state (one value behind one lock; every rule of the trait is a method of this value, so a
//! call is one transaction).

use crate::bin::Bin;
use crate::groups::GroupMembers;
use crate::ids::{invalid, object_id};
use crate::tasks_v1::{
    Change, Error, FieldChange, HistoryEntry, HistoryKind, LocalDateTime, ObjectId, Override,
    PersonRef, Recurrence, Revision, Task, TaskError, TaskList, TaskResult, Utc,
};
use std::collections::BTreeMap;
use std::sync::Arc;

/// The default of the trash setting: 30 days.
pub(crate) const TRASH_DAYS: u32 = 30;

/// A stored task and its trash state.
#[derive(Clone, Debug)]
pub(crate) struct Row {
    pub task: Task,
    /// Own deletion: `Some` = in the trash.
    pub deleted: Option<Bin>,
    /// Deleted together with this parent (comes back only with it).
    pub deleted_with: Option<ObjectId>,
}

/// What a partial delete (`This`, `ThisAndFollowing`) took away, so `restore` can give it back.
#[derive(Clone, Debug)]
pub(crate) struct Cut {
    pub id: ObjectId,
    pub recurrence_id: LocalDateTime,
    pub following: bool,
    pub until: Utc,
    /// The revision right after the cut: a later change makes `restore(ThisAndFollowing)` report `Changed`.
    pub rev_after: Revision,
    pub recurrence: Vec<Recurrence>,
    pub overrides: Vec<Override>,
}

/// Everything the fake knows.
#[derive(Clone, Debug)]
pub(crate) struct State {
    pub lists: Vec<TaskList>,
    pub rows: Vec<Row>,
    pub changes: Vec<Change>,
    pub history: Vec<(ObjectId, HistoryEntry)>,
    pub cuts: Vec<Cut>,
    /// Lists in the trash, with their deletion.
    pub list_bins: BTreeMap<ObjectId, Bin>,
    /// Keeps (`keep_until`) of tasks and lists: `None` = without an end.
    pub keeps: BTreeMap<ObjectId, Option<Utc>>,
    /// Lists a person may only read (person, list): the fixture's way to a `Read` access.
    pub read_only: Vec<(PersonRef, ObjectId)>,
    /// The trash setting: days until the automatic purge, `None` = never ("nie automatisch löschen").
    pub trash_days: Option<u32>,
    pub groups: Arc<dyn GroupMembers>,
    /// The zone setting of the asking person (days of `Date` and floating values in "from completion").
    pub zone: chrono_tz::Tz,
    /// A deliberately wrong behaviour (tests of the rules only).
    pub mutation: Option<crate::mutation::Mutation>,
    pub n: u64,
    pub now: Utc,
    pub account: PersonRef,
    pub me_card: Option<PersonRef>,
    /// Somebody else asks (a colleague, an AI): replaces the account and the "I" card as the asking person.
    pub acting: Option<PersonRef>,
    pub evaluate: bool,
}

/// Compare-and-set: the stored revision must be the expected one.
pub(crate) fn check(current: Revision, expected: Revision) -> TaskResult<()> {
    if current == expected {
        Ok(())
    } else {
        Err(Error::Changed { current }.into())
    }
}

impl State {
    /// Whether the fake runs with this mutation (never, outside the tests of the rules).
    pub(crate) fn mutated(&self, m: crate::mutation::Mutation) -> bool {
        self.mutation == Some(m)
    }

    pub(crate) fn new(
        account: PersonRef,
        now: Utc,
        evaluate: bool,
        groups: Arc<dyn GroupMembers>,
    ) -> Self {
        Self {
            lists: Vec::new(),
            rows: Vec::new(),
            changes: Vec::new(),
            history: Vec::new(),
            cuts: Vec::new(),
            list_bins: BTreeMap::new(),
            keeps: BTreeMap::new(),
            read_only: Vec::new(),
            trash_days: Some(TRASH_DAYS),
            groups,
            zone: chrono_tz::Europe::Berlin,
            mutation: None,
            n: 0,
            now,
            account,
            me_card: None,
            acting: None,
            evaluate,
        }
    }

    /// The asking person: somebody acting, else the "I" card if there is one, else the account of the provider (never
    /// an error).
    pub(crate) fn asker(&self) -> PersonRef {
        self.acting
            .clone()
            .or_else(|| self.me_card.clone())
            .unwrap_or_else(|| self.account.clone())
    }

    /// Whether `who` is the asking person (account or "I" card; somebody acting is only themselves).
    pub(crate) fn is_me(&self, who: &PersonRef) -> bool {
        match &self.acting {
            Some(a) => a.id == who.id,
            None => *who == self.account || self.me_card.as_ref() == Some(who),
        }
    }

    /// One second later; returns the new time.
    pub(crate) fn tick(&mut self) -> Utc {
        self.now = Utc::from_millis(self.now.millis() + 1000);
        self.now
    }

    pub(crate) fn next_id(&mut self) -> ObjectId {
        self.n += 1;
        object_id(self.n)
    }

    pub(crate) fn list_at(&self, id: ObjectId) -> Option<usize> {
        self.lists.iter().position(|l| l.id == id)
    }

    /// A list that lives (not in the trash) and that the asking person may see (else `NotFound`).
    pub(crate) fn live_list(&self, id: ObjectId) -> TaskResult<usize> {
        self.list_at(id)
            .filter(|&i| !self.list_bins.contains_key(&id) && self.sees(&self.lists[i]))
            .ok_or_else(|| Error::NotFound(id).into())
    }

    pub(crate) fn at(&self, id: ObjectId) -> Option<usize> {
        self.rows.iter().position(|r| r.task.id == id)
    }

    /// A task that lives: not deleted, and its list not in the trash.
    pub(crate) fn live(&self, id: ObjectId) -> TaskResult<usize> {
        self.at(id)
            .filter(|&i| self.trash_of(i).is_none() && self.sees_task(i))
            .ok_or_else(|| Error::NotFound(id).into())
    }

    /// Like [`State::live`] with the compare-and-set check.
    pub(crate) fn live_expected(&self, id: ObjectId, expected: Revision) -> TaskResult<usize> {
        let i = self.live(id)?;
        check(self.rows[i].task.revision, expected)?;
        Ok(i)
    }

    /// The living direct subtasks of a task.
    pub(crate) fn children(&self, id: ObjectId) -> Vec<usize> {
        (0..self.rows.len())
            .filter(|&i| self.rows[i].task.parent == Some(id) && self.trash_of(i).is_none())
            .collect()
    }

    /// The living subtasks of a task, all levels, parents before children.
    pub(crate) fn descendants(&self, id: ObjectId) -> Vec<usize> {
        let mut out = Vec::new();
        let mut todo = vec![id];
        while let Some(p) = todo.pop() {
            for c in self.children(p) {
                out.push(c);
                todo.push(self.rows[c].task.id);
            }
        }
        out
    }

    /// An entry in the change feed (for observers).
    pub(crate) fn feed(&mut self, task: ObjectId, list: ObjectId, kind: HistoryKind) {
        let at = self.now;
        self.changes.push(Change {
            task,
            list,
            kind,
            at,
        });
    }

    /// Feed and log of one task by one actor.
    pub(crate) fn note(&mut self, i: usize, kind: HistoryKind, changes: Vec<FieldChange>) {
        let (id, list) = (self.rows[i].task.id, self.rows[i].task.list);
        self.feed(id, list, kind);
        let entry = HistoryEntry {
            at: self.now,
            actor: self.asker(),
            kind,
            changes,
        };
        self.history.push((id, entry));
    }

    /// A write of the acting person: one second later, revision and sequence up, `updated` set, noted.
    pub(crate) fn bump(&mut self, i: usize, kind: HistoryKind, changes: Vec<FieldChange>) {
        let now = self.tick();
        let t = &mut self.rows[i].task;
        t.revision = Revision(t.revision.0 + 1);
        t.sequence += 1;
        t.updated = now;
        self.note(i, kind, changes);
    }

    /// A rank rewritten by the re-spread: revision +1 and a `Move` in the feed, but no entry in the task log (no
    /// human did it) and no change for the caller.
    pub(crate) fn bump_quiet(&mut self, i: usize, kind: HistoryKind) {
        let t = &mut self.rows[i].task;
        t.revision = Revision(t.revision.0 + 1);
        let (id, list) = (t.id, t.list);
        self.feed(id, list, kind);
    }

    /// The `Invalid` error for a field.
    pub(crate) fn bad(field: &str, reason: &str) -> TaskError {
        invalid(field, reason)
    }
}
