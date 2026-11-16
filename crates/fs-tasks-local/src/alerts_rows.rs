//! The rows of the alerts of a task (`alerts`, a map by key): read them for tasks, replace the set of a task.
//!
//! Pattern: Repository helper (rows to values and back; the checks of a patch are in `patch`).

use std::collections::BTreeMap;

use fs_objects::v5;
use fs_objects::v5::codec::{damaged, id_value, int, opt_int, opt_text, text};
use fs_tasks_testdata::tasks_v1::{Alert, ObjectId, TaskResult, Trigger, Utc};

use crate::cur::Cur;
use crate::error::adapt;
use crate::stored::marks;
use crate::tables_more::ALERT;
use crate::words::{action_of, action_word, relative_of, relative_word};

/// The alerts of the given tasks, each task's in the order they were given.
pub async fn of(c: &v5::Conn<'_>, ids: &[ObjectId]) -> TaskResult<BTreeMap<ObjectId, Vec<Alert>>> {
    let mut out: BTreeMap<ObjectId, Vec<Alert>> = BTreeMap::new();
    for chunk in ids.chunks(400) {
        let sql = format!(
            "SELECT task, key, trigger_kind, offset_seconds, relative_to, trigger_at, alert_action, acknowledged \
             FROM tasks_alert WHERE task IN ({}) ORDER BY task, pos",
            marks(chunk.len())
        );
        let rows = c
            .query(&sql, chunk.iter().map(|i| id_value(*i)).collect())
            .await
            .map_err(adapt)?;
        for row in &rows {
            let mut cur = Cur::new(row);
            let (task, key, kind) = (cur.id()?, cur.text()?, cur.text()?);
            let (seconds, relative, at) = (cur.opt_int()?, cur.opt_text()?, cur.opt_int()?);
            let action = action_of(&cur.text()?)?;
            let acknowledged = cur.opt_int()?.map(Utc::from_millis);
            let missing = || adapt(damaged("alert trigger"));
            let trigger = match kind.as_str() {
                "offset" => Trigger::Offset {
                    seconds: seconds.ok_or_else(missing)?,
                    relative_to: relative_of(&relative.ok_or_else(missing)?)?,
                },
                "absolute" => Trigger::Absolute {
                    at: Utc::from_millis(at.ok_or_else(missing)?),
                },
                _ => return Err(missing()),
            };
            out.entry(task).or_default().push(Alert {
                key,
                trigger,
                action,
                acknowledged,
            });
        }
    }
    Ok(out)
}

/// Replace the alerts of a task by `alerts`, in this order.
pub async fn write(c: &v5::Conn<'_>, task: ObjectId, alerts: &[Alert]) -> TaskResult<()> {
    c.exec(
        "DELETE FROM tasks_alert WHERE task = ?",
        vec![id_value(task)],
    )
    .await
    .map_err(adapt)?;
    for (pos, a) in alerts.iter().enumerate() {
        let (kind, seconds, relative, at) = match a.trigger {
            Trigger::Offset {
                seconds,
                relative_to,
            } => (
                "offset",
                opt_int(Some(seconds)),
                opt_text(Some(relative_word(relative_to))),
                opt_int(None),
            ),
            Trigger::Absolute { at } => (
                "absolute",
                opt_int(None),
                opt_text(None),
                opt_int(Some(at.millis())),
            ),
        };
        c.insert(
            &ALERT,
            vec![
                id_value(task),
                text(&a.key),
                int(i64::try_from(pos).unwrap_or(i64::MAX)),
                text(kind),
                seconds,
                relative,
                at,
                text(action_word(a.action)),
                opt_int(a.acknowledged.map(|u| u.millis())),
            ],
        )
        .await
        .map_err(adapt)?;
    }
    Ok(())
}
