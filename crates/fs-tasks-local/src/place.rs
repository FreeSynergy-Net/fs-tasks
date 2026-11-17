//! `place`: the hand order. A task goes on top of its column (`after = None`) or directly after another task of the
//! SAME column (same list, same parent, same progress); anything else is `Invalid`. Only the placed task is written;
//! when no rank fits, the column is spread ONCE in the same transaction (the others get a new revision, the caller
//! does not see a `Changed`).
//!
//! Pattern: Transaction script (in the caller's unit of work).

use fs_objects::v5;
use fs_objects::v5::codec::{id_value, text};
use fs_objects::v5::envelope::{bump, touch};
use fs_tasks_testdata::tasks_v1::{
    ListAccess, ObjectId, Rank, RankError, Revision, Task, TaskResult,
};

use crate::access::need;
use crate::cur::Cur;
use crate::error::{adapt, bad, changed};
use crate::rank::{column, top};
use crate::stored::{Stored, load_one, update_row};
use crate::tasks_write::live_task;
use crate::who::Who;
use crate::words::progress_word;

/// The rank of the first task of the column behind `(rank, id)`, the placed task left out.
async fn next_after(
    c: &v5::Conn<'_>,
    t: &Task,
    rank: &Rank,
    after: ObjectId,
) -> TaskResult<Option<Rank>> {
    let mut sql = "SELECT t.rank FROM tasks_task t JOIN objects o ON o.id = t.id \
        WHERE t.list = ? AND t.progress = ? AND o.deleted IS NULL AND t.deleted_with IS NULL \
        AND t.id <> ? AND (t.rank > ? OR (t.rank = ? AND t.id > ?))"
        .to_owned();
    let mut params = vec![
        id_value(t.list),
        text(progress_word(t.progress)),
        id_value(t.id),
        text(rank.as_str()),
        text(rank.as_str()),
        id_value(after),
    ];
    if let Some(p) = t.parent {
        sql.push_str(" AND t.parent = ?");
        params.push(id_value(p));
    } else {
        sql.push_str(" AND t.parent IS NULL");
    }
    sql.push_str(" ORDER BY t.rank, t.id LIMIT 1");
    let rows = c.query(&sql, params).await.map_err(adapt)?;
    rows.first()
        .map(|r| Rank::parse(&Cur::new(r).text()?).ok_or_else(|| adapt(v5::codec::damaged("rank"))))
        .transpose()
}

/// The rank directly after `after`, spreading the column once when none fits.
async fn rank_after(c: &v5::Conn<'_>, now: i64, t: &Task, after: ObjectId) -> TaskResult<Rank> {
    let anchor = load_one(c, after)
        .await?
        .filter(Stored::alive)
        .map(|s| s.task)
        .filter(|a| a.list == t.list && a.parent == t.parent && a.progress == t.progress)
        .ok_or_else(|| bad("after", "not-in-the-sequence"))?;
    let next = next_after(c, t, &anchor.rank, after).await?;
    match Rank::between(Some(&anchor.rank), next.as_ref()) {
        Ok(r) => Ok(r),
        Err(RankError::Swapped) => Err(bad("rank", "swapped")),
        Err(RankError::Equal | RankError::Exhausted) => {
            let col = column(c, t.list, t.parent, t.progress, Some(t.id), None).await?;
            let spread = Rank::spread(col.len());
            let mut at = 0;
            for (i, ((_, id), rank)) in col.iter().zip(&spread).enumerate() {
                c.exec(
                    "UPDATE tasks_task SET rank = ? WHERE id = ?",
                    vec![text(rank.as_str()), id_value(*id)],
                )
                .await
                .map_err(adapt)?;
                touch(c, *id, None, now).await.map_err(adapt)?;
                if *id == after {
                    at = i;
                }
            }
            Rank::between(spread.get(at), spread.get(at + 1)).map_err(|_| bad("rank", "exhausted"))
        }
    }
}

/// `place`: `NotFound`, `Forbidden`, `Changed`, then `Invalid` (after itself, after a task of another column).
pub async fn place(
    c: &v5::Conn<'_>,
    who: &Who,
    now: i64,
    id: ObjectId,
    expected: Revision,
    after: Option<ObjectId>,
) -> TaskResult<Task> {
    let old = live_task(c, id).await?.task;
    need(c, who, old.list, ListAccess::Write, "task.place").await?;
    if old.revision != expected {
        return Err(changed(old.revision));
    }
    if after == Some(id) {
        return Err(bad("after", "itself"));
    }
    let rank = match after {
        None => top(c, now, old.list, old.parent, old.progress, Some(id)).await?,
        Some(x) => rank_after(c, now, &old, x).await?,
    };
    let mut new = old.clone();
    new.rank = rank;
    bump(c, id, expected, None, now).await.map_err(adapt)?;
    update_row(c, &new, None).await?;
    Ok(live_task(c, id).await?.task)
}
