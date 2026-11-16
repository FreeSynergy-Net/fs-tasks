//! The rows of the dependencies of a task: read them for tasks, write one, remove one.
//!
//! Pattern: Repository helper (rows to values and back; the rules and the cycle check are in `deps`).

use std::collections::BTreeMap;

use fs_objects::v5;
use fs_objects::v5::codec::{boolean, id_value, int, text};
use fs_tasks_testdata::tasks_v1::{Dependency, Gap, ObjectId, Span, TaskResult};
use turso::Value;

use crate::cur::Cur;
use crate::error::adapt;
use crate::stored::marks;
use crate::tables_more::DEPENDENCY;
use crate::words::{kind_of, kind_word};

/// The dependencies of the given tasks, each task's in the order they were given.
pub async fn of(
    c: &v5::Conn<'_>,
    ids: &[ObjectId],
) -> TaskResult<BTreeMap<ObjectId, Vec<Dependency>>> {
    let mut out: BTreeMap<ObjectId, Vec<Dependency>> = BTreeMap::new();
    for chunk in ids.chunks(400) {
        let sql = format!(
            "SELECT task, other_uid, kind, gap_negative, gap_days, gap_seconds FROM tasks_dependency \
             WHERE task IN ({}) ORDER BY task, pos",
            marks(chunk.len())
        );
        let rows = c
            .query(&sql, chunk.iter().map(|i| id_value(*i)).collect())
            .await
            .map_err(adapt)?;
        for row in &rows {
            let mut cur = Cur::new(row);
            let task = cur.id()?;
            let (other_uid, kind) = (cur.text()?, kind_of(&cur.text()?)?);
            let (negative, days, seconds) = (cur.opt_int()?, cur.opt_int()?, cur.opt_int()?);
            let gap = negative.zip(days).zip(seconds).map(|((n, d), s)| Gap {
                negative: n != 0,
                span: Span {
                    days: u32::try_from(d).unwrap_or(u32::MAX),
                    seconds: u32::try_from(s).unwrap_or(u32::MAX),
                },
            });
            out.entry(task).or_default().push(Dependency {
                other_uid,
                kind,
                gap,
            });
        }
    }
    Ok(out)
}

/// Add one dependency after the others of the task.
pub async fn add(c: &v5::Conn<'_>, task: ObjectId, pos: i64, d: &Dependency) -> TaskResult<()> {
    let (neg, days, secs) = d.gap.map_or((Value::Null, Value::Null, Value::Null), |g| {
        (
            boolean(g.negative),
            int(i64::from(g.span.days)),
            int(i64::from(g.span.seconds)),
        )
    });
    c.insert(
        &DEPENDENCY,
        vec![
            id_value(task),
            int(pos),
            text(&d.other_uid),
            text(kind_word(d.kind)),
            neg,
            days,
            secs,
        ],
    )
    .await
    .map_err(adapt)
}

/// Remove the dependency on `other_uid` of this kind.
pub async fn remove(
    c: &v5::Conn<'_>,
    task: ObjectId,
    other_uid: &str,
    kind: fs_tasks_testdata::tasks_v1::DependencyKind,
) -> TaskResult<()> {
    c.exec(
        "DELETE FROM tasks_dependency WHERE task = ? AND other_uid = ? AND kind = ?",
        vec![id_value(task), text(other_uid), text(kind_word(kind))],
    )
    .await
    .map_err(adapt)?;
    Ok(())
}
