//! A task as stored: the trait's `Task` plus what only the store knows (the trash state of the task, of the task it was
//! deleted with, and of its list), read with one `SELECT` over the envelope.
//!
//! Pattern: Repository helper (rows to values; the effective trash state is derived, never stored twice).

use fs_objects::v5;
use fs_objects::v5::codec::{id_value, int, opt_text, text};
use fs_tasks_testdata::tasks_v1::{
    ObjectId, Rank, Restorable, Revision, Span, Task, TaskResult, Utc,
};
use turso::Value;

use crate::cur::Cur;
use crate::due;
use crate::error::adapt;
use crate::tables::{LABEL, TASK};
use crate::trash_state::{Gone, gone};
use crate::words::{progress_of, progress_word, text_type_of, text_type_word};

/// The columns of a task row and of its envelope, in the order `read` takes them.
const COLUMNS: &str = "t.id, t.list, t.uid, t.parent, t.deleted_with, t.sequence, t.title, t.description, \
    t.description_type, t.locale, t.due_kind, t.due_date, t.due_local, t.due_zone, t.due_zone_assumed, \
    t.start_kind, t.start_date, t.start_local, t.start_zone, t.start_zone_assumed, t.estimated_days, \
    t.estimated_seconds, t.progress, t.progress_updated, t.percent_complete, t.priority, t.color, t.rank, \
    o.rev, o.published, o.updated, o.deleted, o.purge_after, lo.deleted, lo.purge_after, wo.deleted, wo.purge_after";

const FROM: &str = "FROM tasks_task t JOIN objects o ON o.id = t.id JOIN objects lo ON lo.id = t.list \
    LEFT JOIN objects wo ON wo.id = t.deleted_with";

/// A task with its trash state.
#[derive(Clone, Debug)]
pub struct Stored {
    pub task: Task,
    /// The task was deleted on its own.
    pub own: Option<Gone>,
    /// The task it was deleted WITH (a subtask), and that one's deadline.
    pub with: Option<(ObjectId, Gone)>,
    /// The list is in the trash.
    pub list: Option<Gone>,
}

impl Stored {
    /// The trash state that counts: the task's own, else that of the task it went with, else its list's.
    #[must_use]
    pub fn trash(&self) -> Option<Gone> {
        self.own.or(self.with.map(|w| w.1)).or(self.list)
    }

    /// `None` = alive; `Some` = in the trash with the deadline.
    #[must_use]
    pub fn restorable(&self) -> Option<Restorable> {
        self.trash().map(Gone::restorable)
    }

    /// Alive: not deleted on its own, not with a parent, the list is not in the trash.
    #[must_use]
    pub fn alive(&self) -> bool {
        self.trash().is_none()
    }
}

/// Load the tasks that match `cond` (SQL over the aliases `t`, `o`, `lo`, `wo`), by id, with their labels.
pub async fn load(c: &v5::Conn<'_>, cond: &str, params: Vec<Value>) -> TaskResult<Vec<Stored>> {
    load_with(c, cond, params, "ORDER BY t.id").await
}

/// [`load`] with the end of the statement (`ORDER BY .. LIMIT .. OFFSET ..`) in the caller's hands: the order and the
/// page are the database's work.
pub async fn load_with(
    c: &v5::Conn<'_>,
    cond: &str,
    params: Vec<Value>,
    tail: &str,
) -> TaskResult<Vec<Stored>> {
    let sql = format!("SELECT {COLUMNS} {FROM} WHERE {cond} {tail}");
    let rows = c.query(&sql, params).await.map_err(adapt)?;
    let mut out = Vec::new();
    for row in &rows {
        out.push(read(&mut Cur::new(row))?);
    }
    let ids: Vec<ObjectId> = out.iter().map(|s| s.task.id).collect();
    let labels = labels_of(c, &ids).await?;
    let mut items = crate::checklist_rows::items_of(c, &ids).await?;
    for s in &mut out {
        s.task.labels = labels
            .iter()
            .filter(|(t, _)| *t == s.task.id)
            .map(|(_, v)| v.clone())
            .collect();
        s.task.checklist = items.remove(&s.task.id).unwrap_or_default();
    }
    crate::parts::attach(c, &mut out).await?;
    Ok(out)
}

/// One task by id, in any trash state.
pub async fn load_one(c: &v5::Conn<'_>, id: ObjectId) -> TaskResult<Option<Stored>> {
    Ok(load(c, "t.id = ?", vec![id_value(id)])
        .await?
        .into_iter()
        .next())
}

/// `?, ?, ?` for `n` parameters.
#[must_use]
pub fn marks(n: usize) -> String {
    vec!["?"; n].join(", ")
}

/// The labels of the given tasks, sorted by value (a set).
async fn labels_of(c: &v5::Conn<'_>, ids: &[ObjectId]) -> TaskResult<Vec<(ObjectId, String)>> {
    let mut out = Vec::new();
    for chunk in ids.chunks(400) {
        let sql = format!(
            "SELECT task, value FROM tasks_label WHERE task IN ({}) ORDER BY value",
            marks(chunk.len())
        );
        let rows = c
            .query(&sql, chunk.iter().map(|i| id_value(*i)).collect())
            .await
            .map_err(adapt)?;
        for row in &rows {
            let mut cur = Cur::new(row);
            out.push((cur.id()?, cur.text()?));
        }
    }
    Ok(out)
}

fn read(cur: &mut Cur<'_>) -> TaskResult<Stored> {
    let id = cur.id()?;
    let list = cur.id()?;
    let uid = cur.text()?;
    let parent = cur.opt_id()?;
    let with_id = cur.opt_id()?;
    let sequence = cur.int()?;
    let title = cur.text()?;
    let description = cur.text()?;
    let description_type = text_type_of(&cur.text()?)?;
    let locale = cur.opt_text()?;
    let due = due::read(cur)?;
    let start = due::read(cur)?;
    let estimated = match (cur.opt_int()?, cur.opt_int()?) {
        (Some(d), Some(s)) => Some(Span {
            days: u32::try_from(d).unwrap_or(u32::MAX),
            seconds: u32::try_from(s).unwrap_or(u32::MAX),
        }),
        _ => None,
    };
    let progress = progress_of(&cur.text()?)?;
    let progress_updated = cur.opt_int()?.map(Utc::from_millis);
    let percent_complete = cur.opt_int()?.and_then(|p| u8::try_from(p).ok());
    let priority = u8::try_from(cur.int()?).unwrap_or(0);
    let color = cur.opt_text()?;
    let rank = Rank::parse(&cur.text()?).ok_or_else(|| adapt(damaged_rank()))?;
    let revision = Revision(u64::try_from(cur.int()?).unwrap_or(0));
    let (created, updated) = (Utc::from_millis(cur.int()?), Utc::from_millis(cur.int()?));
    let own = gone(cur.opt_int()?, cur.opt_int()?);
    let list_gone = gone(cur.opt_int()?, cur.opt_int()?);
    let with_gone = gone(cur.opt_int()?, cur.opt_int()?);
    let mut task = blank(id, list, &uid, &title, created);
    task.parent = parent;
    task.sequence = u32::try_from(sequence).unwrap_or(0);
    task.description = description;
    task.description_type = description_type;
    task.locale = locale;
    task.due = due;
    task.start = start;
    task.estimated = estimated;
    task.progress = progress;
    task.progress_updated = progress_updated;
    task.percent_complete = percent_complete;
    task.priority = priority;
    task.color = color;
    task.rank = rank;
    task.revision = revision;
    task.updated = updated;
    Ok(Stored {
        task,
        own,
        with: with_id.zip(with_gone),
        list: list_gone,
    })
}

fn damaged_rank() -> v5::Error {
    fs_objects::v5::codec::damaged("rank")
}

fn blank(id: ObjectId, list: ObjectId, uid: &str, title: &str, at: Utc) -> Task {
    fs_tasks_testdata::blank_task(id, list, uid, title, at)
}

/// The column values of a task row, in the order of `tables::TASK` (`with`: the task it was deleted with).
pub fn row_values(t: &Task, with: Option<ObjectId>) -> TaskResult<Vec<Value>> {
    let mut v = vec![
        id_value(t.id),
        id_value(t.list),
        text(&t.uid),
        t.parent.map_or(Value::Null, id_value),
        with.map_or(Value::Null, id_value),
        int(i64::from(t.sequence)),
        text(&t.title),
        text(&t.description),
        text(text_type_word(t.description_type)),
        opt_text(t.locale.as_deref()),
    ];
    v.extend(due::values(t.due.as_ref())?);
    v.extend(due::values(t.start.as_ref())?);
    let (days, seconds) = t.estimated.map_or((Value::Null, Value::Null), |s| {
        (int(i64::from(s.days)), int(i64::from(s.seconds)))
    });
    v.extend([
        days,
        seconds,
        text(progress_word(t.progress)),
        t.progress_updated.map_or(Value::Null, |u| int(u.millis())),
        t.percent_complete
            .map_or(Value::Null, |p| int(i64::from(p))),
        int(i64::from(t.priority)),
        opt_text(t.color.as_deref()),
        text(t.rank.as_str()),
    ]);
    Ok(v)
}

/// Write the whole row of a task (everything but `id`) back; the envelope is the caller's (`bump`).
pub async fn update_row(c: &v5::Conn<'_>, t: &Task, with: Option<ObjectId>) -> TaskResult<()> {
    let set: Vec<String> = TASK.cols[1..]
        .iter()
        .map(|col| format!("{} = ?", col.name))
        .collect();
    let mut values = row_values(t, with)?;
    values.remove(0);
    values.push(id_value(t.id));
    c.exec(
        &format!("UPDATE tasks_task SET {} WHERE id = ?", set.join(", ")),
        values,
    )
    .await
    .map_err(adapt)?;
    Ok(())
}

/// Replace the label set of a task.
pub async fn write_labels(c: &v5::Conn<'_>, id: ObjectId, labels: &[String]) -> TaskResult<()> {
    c.exec("DELETE FROM tasks_label WHERE task = ?", vec![id_value(id)])
        .await
        .map_err(adapt)?;
    for label in labels {
        c.insert(&LABEL, vec![id_value(id), text(label)])
            .await
            .map_err(adapt)?;
    }
    Ok(())
}
