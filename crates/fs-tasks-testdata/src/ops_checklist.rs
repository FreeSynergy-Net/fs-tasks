//! The checklist of a task: add, rename, tick, move and remove an item (one change = one transaction on the task).
//!
//! Pattern: Command methods of the state (check everything first, then write once). An item is a row of the task, not an
//! object: it has no revision of its own, every change bumps the revision of the task and writes one log entry whose
//! `row_key` is the key of the item. A change that changes nothing writes nothing.

use crate::mutation::Mutation;
use crate::state::{State, check};
use crate::tasks_v1::{
    CHECKLIST_MAX, Checked, ChecklistChange, ChecklistItem, FieldChange, HistoryKind, ObjectId,
    Rank, RankError, Revision, Task, TaskResult, Value,
};

fn line(field: &str, key: &str, old: Option<Value>, new: Option<Value>) -> FieldChange {
    FieldChange {
        field: field.to_owned(),
        row_key: Some(key.to_owned()),
        old,
        new,
    }
}

fn pos(items: &[ChecklistItem], key: &str) -> TaskResult<usize> {
    items
        .iter()
        .position(|c| c.key == key)
        .ok_or_else(|| State::bad("checklist.key", "unknown"))
}

/// A rank right after the item `after` (`None` = at the top) among `items` without the item `moving`. When it runs
/// out, the ranks of the items are spread once in the same transaction (only this task's items).
fn rank_between(
    items: &mut [ChecklistItem],
    after: Option<&str>,
    moving: Option<&str>,
) -> TaskResult<Rank> {
    for _ in 0..2 {
        let others: Vec<usize> = (0..items.len())
            .filter(|&i| Some(items[i].key.as_str()) != moving)
            .collect();
        let at = match after {
            None => 0,
            Some(k) => {
                others
                    .iter()
                    .position(|&i| items[i].key == k)
                    .ok_or_else(|| State::bad("checklist.key", "unknown"))?
                    + 1
            }
        };
        let before = (at > 0).then(|| items[others[at - 1]].rank.clone());
        let next = others.get(at).map(|&i| items[i].rank.clone());
        match Rank::between(before.as_ref(), next.as_ref()) {
            Ok(r) => return Ok(r),
            Err(RankError::Swapped) => return Err(State::bad("checklist.rank", "swapped")),
            Err(RankError::Equal | RankError::Exhausted) => {
                for (&i, r) in others.iter().zip(Rank::spread(others.len())) {
                    items[i].rank = r;
                }
            }
        }
    }
    Err(State::bad("checklist.rank", "exhausted"))
}

/// The items of a new task: the titles in this order, all open (`Invalid` for an empty title or too many).
pub(crate) fn items_of(
    titles: &[String],
    keys: &mut impl FnMut() -> String,
) -> TaskResult<Vec<ChecklistItem>> {
    if titles.len() > CHECKLIST_MAX {
        return Err(State::bad("checklist", "too-many"));
    }
    if titles.iter().any(|t| t.trim().is_empty()) {
        return Err(State::bad("checklist.title", "empty"));
    }
    Ok(titles
        .iter()
        .zip(Rank::spread(titles.len()))
        .map(|(title, rank)| ChecklistItem {
            key: keys(),
            title: title.clone(),
            done: None,
            rank,
        })
        .collect())
}

impl State {
    pub(crate) fn next_key(&mut self) -> String {
        self.n += 1;
        format!("k{}", self.n)
    }

    pub(crate) fn checklist(
        &mut self,
        id: ObjectId,
        expected: Revision,
        change: ChecklistChange,
    ) -> TaskResult<Task> {
        self.expire();
        let i = self.live(id)?;
        self.need_write(self.rows[i].task.list, "task.checklist")?;
        check(self.rows[i].task.revision, expected)?;
        let mut items = self.rows[i].task.checklist.clone();
        let (kind, fields) = match change {
            ChecklistChange::Add { title, after } => {
                if title.trim().is_empty() {
                    return Err(Self::bad("checklist.title", "empty"));
                }
                if items.len() >= CHECKLIST_MAX && !self.mutated(Mutation::ChecklistNoLimit) {
                    return Err(Self::bad("checklist", "too-many"));
                }
                let rank = rank_between(&mut items, after.as_deref(), None)?;
                let key = self.next_key();
                let new = Some(Value::Text(title.clone()));
                items.push(ChecklistItem {
                    key: key.clone(),
                    title,
                    done: None,
                    rank,
                });
                (HistoryKind::Add, vec![line("checklist", &key, None, new)])
            }
            ChecklistChange::Rename { key, title } => {
                let at = pos(&items, &key)?;
                if title.trim().is_empty() {
                    return Err(Self::bad("checklist.title", "empty"));
                }
                if items[at].title == title {
                    return Ok(self.rows[i].task.clone());
                }
                let old = Some(Value::Text(std::mem::replace(
                    &mut items[at].title,
                    title.clone(),
                )));
                (
                    HistoryKind::Update,
                    vec![line("checklist.title", &key, old, Some(Value::Text(title)))],
                )
            }
            ChecklistChange::Check { key, done } => {
                let at = pos(&items, &key)?;
                if items[at].done.is_some() == done {
                    return Ok(self.rows[i].task.clone());
                }
                items[at].done = done.then(|| Checked {
                    by: self.asker(),
                    at: self.now,
                });
                (
                    HistoryKind::Update,
                    vec![line(
                        "checklist.done",
                        &key,
                        Some(Value::Bool(!done)),
                        Some(Value::Bool(done)),
                    )],
                )
            }
            ChecklistChange::Place { key, after } => {
                let at = pos(&items, &key)?;
                if after.as_deref() == Some(key.as_str()) {
                    return Err(Self::bad("checklist.key", "itself"));
                }
                let old = items[at].rank.clone();
                let rank = rank_between(&mut items, after.as_deref(), Some(&key))?;
                let at = pos(&items, &key)?;
                items[at].rank = rank.clone();
                let others_moved = items
                    .iter()
                    .zip(&self.rows[i].task.checklist)
                    .any(|(a, b)| a.key != key && a.rank != b.rank);
                if rank == old && !others_moved {
                    return Ok(self.rows[i].task.clone());
                }
                let (o, n) = (
                    Some(Value::Text(old.as_str().to_owned())),
                    Some(Value::Text(rank.as_str().to_owned())),
                );
                (HistoryKind::Move, vec![line("checklist.rank", &key, o, n)])
            }
            ChecklistChange::Remove { key } => {
                let at = pos(&items, &key)?;
                let gone = items.remove(at);
                (
                    HistoryKind::Remove,
                    vec![line("checklist", &key, Some(Value::Text(gone.title)), None)],
                )
            }
        };
        items.sort_by(|a, b| a.rank.cmp(&b.rank));
        self.rows[i].task.checklist = items;
        self.bump(i, kind, fields);
        Ok(self.rows[i].task.clone())
    }
}
