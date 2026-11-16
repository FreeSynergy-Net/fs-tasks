//! The checklist of a task: read, write the draft's items, and the five changes (`Add`, `Rename`, `Check`, `Place`,
//! `Remove`). An item is only text, a tick and a place: no due date, no assignee, no revision of its own; every change
//! raises the revision of the TASK. The keys are `k1`, `k2` .. (the highest number plus one), the order is a rank
//! inside the task (the rules of `Rank`; a rank that runs out spreads the items of that task once, in the same
//! transaction).
//!
//! Pattern: Transaction scripts (in the caller's unit of work).

use fs_objects::v5;
use fs_objects::v5::codec::{id_value, int, text};
use fs_objects::v5::envelope::bump;
use fs_tasks_testdata::tasks_v1::{
    CHECKLIST_MAX, ChecklistChange, ChecklistItem, ListAccess, ObjectId, Rank, RankError, Revision,
    Task, TaskResult,
};
use turso::Value;

use crate::access::need;
use crate::checklist_rows::{check_title, next_key, row};
use crate::error::{adapt, bad, changed};
use crate::tables::CHECKLIST;
use crate::tasks_write::live_task;
use crate::who::Who;

fn position(items: &[ChecklistItem], key: &str) -> TaskResult<usize> {
    items
        .iter()
        .position(|i| i.key == key)
        .ok_or_else(|| bad("checklist.key", "unknown"))
}

/// The rank for a place behind `after` (`None` = on top) among `items`; `Err(true)` = nothing fits, spread first.
fn rank_behind(items: &[&ChecklistItem], after: Option<usize>) -> Result<Rank, bool> {
    let (low, high) = match after {
        None => (None, items.first().map(|i| &i.rank)),
        Some(i) => (Some(&items[i].rank), items.get(i + 1).map(|n| &n.rank)),
    };
    Rank::between(low, high).map_err(|e| matches!(e, RankError::Equal | RankError::Exhausted))
}

/// Spread the ranks of the items of a task once.
async fn spread(c: &v5::Conn<'_>, task: ObjectId, items: &mut [ChecklistItem]) -> TaskResult<()> {
    let ranks = Rank::spread(items.len());
    for (i, rank) in items.iter_mut().zip(ranks) {
        i.rank = rank;
        c.exec(
            "UPDATE tasks_checklist SET rank = ? WHERE task = ? AND key = ?",
            vec![text(i.rank.as_str()), id_value(task), text(&i.key)],
        )
        .await
        .map_err(adapt)?;
    }
    Ok(())
}

/// The rank behind `after` among `items` without the one at `skip`; spreads once when none fits. Returns the rank.
async fn place_rank(
    c: &v5::Conn<'_>,
    task: ObjectId,
    items: &mut [ChecklistItem],
    skip: Option<usize>,
    after: Option<&str>,
) -> TaskResult<Rank> {
    let after_key = after.map(|k| position(items, k)).transpose()?;
    if after_key.is_some() && after_key == skip {
        return Err(bad("checklist.key", "after-itself"));
    }
    for round in 0..2 {
        let others: Vec<&ChecklistItem> = items
            .iter()
            .enumerate()
            .filter(|(n, _)| Some(*n) != skip)
            .map(|(_, i)| i)
            .collect();
        let at = after_key.map(|a| {
            others
                .iter()
                .position(|i| i.key == items[a].key)
                .unwrap_or(0)
        });
        match rank_behind(&others, at) {
            Ok(r) => return Ok(r),
            Err(true) if round == 0 => spread(c, task, items).await?,
            Err(_) => return Err(bad("checklist", "rank")),
        }
    }
    Err(bad("checklist", "rank"))
}

/// `checklist`: `NotFound`, `Forbidden`, `Changed`, then `Invalid`; a change that changes nothing (ticking a ticked
/// item, the same title) writes nothing and keeps the revision.
pub async fn change(
    c: &v5::Conn<'_>,
    who: &Who,
    now: i64,
    id: ObjectId,
    expected: Revision,
    change: ChecklistChange,
) -> TaskResult<Task> {
    let old = live_task(c, id).await?.task;
    need(c, who, old.list, ListAccess::Write, "task.checklist").await?;
    if old.revision != expected {
        return Err(changed(old.revision));
    }
    let mut items = old.checklist.clone();
    let wrote = apply(c, who, now, id, &mut items, change).await?;
    if !wrote {
        return Ok(old);
    }
    bump(c, id, expected, None, now).await.map_err(adapt)?;
    Ok(live_task(c, id).await?.task)
}

/// Run one change against the stored items; `false` = nothing to write.
async fn apply(
    c: &v5::Conn<'_>,
    who: &Who,
    now: i64,
    task: ObjectId,
    items: &mut [ChecklistItem],
    change: ChecklistChange,
) -> TaskResult<bool> {
    match change {
        ChecklistChange::Add { title, after } => {
            check_title(&title)?;
            if items.len() >= CHECKLIST_MAX {
                return Err(bad("checklist", "too-many"));
            }
            let rank = place_rank(c, task, items, None, after.as_deref()).await?;
            let item = ChecklistItem {
                key: next_key(c, task, items).await?,
                title,
                done: None,
                rank,
            };
            c.insert(&CHECKLIST, row(task, &item))
                .await
                .map_err(adapt)?;
            Ok(true)
        }
        ChecklistChange::Rename { key, title } => {
            let i = position(items, &key)?;
            check_title(&title)?;
            set(
                c,
                task,
                &key,
                "title = ?",
                vec![text(&title)],
                items[i].title != title,
            )
            .await
        }
        ChecklistChange::Check { key, done } => {
            let i = position(items, &key)?;
            if items[i].done.is_some() == done {
                return Ok(false);
            }
            let sets = if done {
                let [id, art, abzug] = v5::codec::person_values(&who.me);
                (
                    "done_by_id = ?, done_by_art = ?, done_by_abzug = ?, done_at = ?",
                    vec![id, art, abzug, int(now)],
                )
            } else {
                (
                    "done_by_id = NULL, done_by_art = NULL, done_by_abzug = NULL, done_at = NULL",
                    Vec::new(),
                )
            };
            set(c, task, &key, sets.0, sets.1, true).await
        }
        ChecklistChange::Place { key, after } => {
            let i = position(items, &key)?;
            let rank = place_rank(c, task, items, Some(i), after.as_deref()).await?;
            let i = position(items, &key)?;
            set(
                c,
                task,
                &key,
                "rank = ?",
                vec![text(rank.as_str())],
                items[i].rank != rank,
            )
            .await
        }
        ChecklistChange::Remove { key } => {
            position(items, &key)?;
            c.exec(
                "DELETE FROM tasks_checklist WHERE task = ? AND key = ?",
                vec![id_value(task), text(&key)],
            )
            .await
            .map_err(adapt)?;
            Ok(true)
        }
    }
}

/// `UPDATE tasks_checklist SET <sets> WHERE task AND key` when `really`; the answer says whether it wrote.
async fn set(
    c: &v5::Conn<'_>,
    task: ObjectId,
    key: &str,
    sets: &str,
    mut params: Vec<Value>,
    really: bool,
) -> TaskResult<bool> {
    if !really {
        return Ok(false);
    }
    params.extend([id_value(task), text(key)]);
    c.exec(
        &format!("UPDATE tasks_checklist SET {sets} WHERE task = ? AND key = ?"),
        params,
    )
    .await
    .map_err(adapt)?;
    Ok(true)
}
