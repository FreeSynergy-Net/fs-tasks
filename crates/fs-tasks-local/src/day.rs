//! The derived day columns of a task (`tasks_day`): written from its due and start values on every write, and filled
//! for the tasks of a file that had none (step 6). A plain date and a floating time count by their own day, a timed
//! value by its instant: its day depends on the zone the asker views it in, so the range of days of a query becomes
//! bounds on the instant (`due_sql.rs`).
//!
//! Pattern: Derived columns (R11: computed by code on every write, never accepted from a caller).

use fs_objects::v5;
use fs_objects::v5::codec::{date_to_int, id_value, int};
use fs_objects::v5::migrate::StepFuture;
use fs_tasks_testdata::tasks_v1::{Due, ObjectId, Task, TaskResult};
use turso::Value;

use crate::error::adapt;
use crate::stored::load;
use crate::tables_more::DAY;
use crate::zones::instant;

/// The two derived values of one due or start value: its own day (plain, floating) or its instant in milliseconds (timed;
/// none when the local time does not exist in its zone, so it is on no day).
#[must_use]
pub fn parts(due: Option<&Due>) -> (Option<i64>, Option<i64>) {
    match due {
        Some(Due::Date(d)) => (Some(date_to_int(d)), None),
        Some(Due::Floating(l)) => (Some(date_to_int(&l.date())), None),
        Some(d @ Due::Timed { .. }) => (None, instant(d)),
        None => (None, None),
    }
}

fn value(v: Option<i64>) -> Value {
    v.map_or(Value::Null, int)
}

/// Write the day columns of a task (one row; the old one goes).
pub async fn write(c: &v5::Conn<'_>, t: &Task) -> TaskResult<()> {
    c.exec("DELETE FROM tasks_day WHERE task = ?", vec![id_value(t.id)])
        .await
        .map_err(adapt)?;
    let (due_day, due_utc) = parts(t.due.as_ref());
    let (start_day, start_utc) = parts(t.start.as_ref());
    c.insert(
        &DAY,
        vec![
            id_value(t.id),
            value(due_day),
            value(due_utc),
            value(start_day),
            value(start_utc),
        ],
    )
    .await
    .map_err(adapt)
}

/// Step 6: the day columns of every task that has no row yet (a file that was made before step 5). The same function
/// as the writes, so the old and the new rows cannot differ.
pub fn fill<'a>(c: &'a v5::Conn<'_>) -> StepFuture<'a> {
    Box::pin(async move {
        let ids: Vec<ObjectId> = {
            let rows = c
                .query(
                    "SELECT t.id FROM tasks_task t WHERE NOT EXISTS (SELECT 1 FROM tasks_day d WHERE d.task = t.id)",
                    Vec::new(),
                )
                .await?;
            let mut ids = Vec::new();
            for r in &rows {
                ids.push(v5::codec::get_id(r, 0)?);
            }
            ids
        };
        for id in ids {
            let found = load(c, "t.id = ?", vec![id_value(id)])
                .await
                .map_err(|e| v5::Error::Unavailable(format!("day columns: {e:?}")))?;
            for s in found {
                write(c, &s.task)
                    .await
                    .map_err(|e| v5::Error::Unavailable(format!("day columns: {e:?}")))?;
            }
        }
        Ok(())
    })
}
