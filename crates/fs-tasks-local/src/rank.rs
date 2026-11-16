//! Hand order: the rank of a new or moved task, at the top of its sequence (list, parent, progress).
//!
//! Pattern: Transaction script (in the caller's transaction). A rank is the fractional text of the trait (`Rank`);
//! the normal case writes only the task itself. When no rank fits between the top and nothing (`Equal`, `Exhausted`),
//! the sequence is spread ONCE (`Rank::spread`) in the same transaction; those tasks get a new revision.

use fs_objects::v5;
use fs_objects::v5::codec::{id_value, text};
use fs_objects::v5::envelope::touch;
use fs_tasks_testdata::tasks_v1::{ObjectId, Progress, Rank, RankError, TaskResult};

use crate::cur::Cur;
use crate::error::{adapt, bad};
use crate::words::progress_word;

/// The living tasks of a sequence with their ranks, in order.
pub async fn column(
    c: &v5::Conn<'_>,
    list: ObjectId,
    parent: Option<ObjectId>,
    progress: Progress,
    exclude: Option<ObjectId>,
    limit: Option<usize>,
) -> TaskResult<Vec<(Rank, ObjectId)>> {
    let mut sql = "SELECT t.rank, t.id FROM tasks_task t JOIN objects o ON o.id = t.id \
        WHERE t.list = ? AND t.progress = ? AND o.deleted IS NULL AND t.deleted_with IS NULL"
        .to_owned();
    let mut params = vec![id_value(list), text(progress_word(progress))];
    match parent {
        Some(p) => {
            sql.push_str(" AND t.parent = ?");
            params.push(id_value(p));
        }
        None => sql.push_str(" AND t.parent IS NULL"),
    }
    if let Some(x) = exclude {
        sql.push_str(" AND t.id <> ?");
        params.push(id_value(x));
    }
    sql.push_str(" ORDER BY t.rank, t.id");
    if let Some(n) = limit {
        sql.push_str(" LIMIT ");
        sql.push_str(&n.to_string());
    }
    let rows = c.query(&sql, params).await.map_err(adapt)?;
    let mut out = Vec::new();
    for row in &rows {
        let mut cur = Cur::new(row);
        let rank = Rank::parse(&cur.text()?)
            .ok_or_else(|| adapt(fs_objects::v5::codec::damaged("rank")))?;
        out.push((rank, cur.id()?));
    }
    Ok(out)
}

/// A rank above everything in the sequence (the new card is on top). `exclude` is the task that is being moved. The
/// normal case reads ONE row (the first of the column); only a rank that runs out loads the whole column to spread it.
pub async fn top(
    c: &v5::Conn<'_>,
    now: i64,
    list: ObjectId,
    parent: Option<ObjectId>,
    progress: Progress,
    exclude: Option<ObjectId>,
) -> TaskResult<Rank> {
    let first = column(c, list, parent, progress, exclude, Some(1)).await?;
    match Rank::between(None, first.first().map(|f| &f.0)) {
        Ok(rank) => Ok(rank),
        Err(RankError::Swapped) => Err(bad("rank", "swapped")),
        Err(RankError::Equal | RankError::Exhausted) => {
            let col = column(c, list, parent, progress, exclude, None).await?;
            let spread = Rank::spread(col.len());
            for ((_, id), rank) in col.iter().zip(&spread) {
                c.exec(
                    "UPDATE tasks_task SET rank = ? WHERE id = ?",
                    vec![text(rank.as_str()), id_value(*id)],
                )
                .await
                .map_err(adapt)?;
                touch(c, *id, None, now).await.map_err(adapt)?;
            }
            Rank::between(None, spread.first()).map_err(|_| bad("rank", "exhausted"))
        }
    }
}
