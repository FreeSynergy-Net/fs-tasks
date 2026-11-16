//! The rows of the checklist: read the items of tasks, write the items of a draft, the row of one item.
//!
//! Pattern: Repository helper (rows to values and back; the changes are `checklist`).

use std::collections::BTreeMap;

use fs_objects::v5;
use fs_objects::v5::codec::{id_value, int, text};
use fs_tasks_testdata::tasks_v1::{
    CHECKLIST_MAX, Checked, ChecklistItem, ObjectId, Rank, TaskResult, Utc,
};
use turso::Value;

use crate::cur::Cur;
use crate::error::{adapt, bad};
use crate::stored::marks;
use crate::tables::{CHECKLIST, CHECKLIST_SEQ};

/// The items of the given tasks, each task's items in their order.
pub async fn items_of(
    c: &v5::Conn<'_>,
    ids: &[ObjectId],
) -> TaskResult<BTreeMap<ObjectId, Vec<ChecklistItem>>> {
    let mut out: BTreeMap<ObjectId, Vec<ChecklistItem>> = BTreeMap::new();
    for chunk in ids.chunks(400) {
        let sql = format!(
            "SELECT task, key, title, rank, done_by_id, done_by_art, done_by_abzug, done_at \
             FROM tasks_checklist WHERE task IN ({}) ORDER BY rank, key",
            marks(chunk.len())
        );
        let rows = c
            .query(&sql, chunk.iter().map(|i| id_value(*i)).collect())
            .await
            .map_err(adapt)?;
        for row in &rows {
            let mut cur = Cur::new(row);
            let task = cur.id()?;
            let (key, title) = (cur.text()?, cur.text()?);
            let rank =
                Rank::parse(&cur.text()?).ok_or_else(|| adapt(v5::codec::damaged("rank")))?;
            let by = cur.opt_person()?;
            let at = cur.opt_int()?;
            let done = by.zip(at).map(|(by, at)| Checked {
                by,
                at: Utc::from_millis(at),
            });
            out.entry(task).or_default().push(ChecklistItem {
                key,
                title,
                done,
                rank,
            });
        }
    }
    Ok(out)
}

pub(crate) fn row(task: ObjectId, i: &ChecklistItem) -> Vec<Value> {
    let (by, at) = match &i.done {
        Some(d) => {
            let [id, art, abzug] = v5::codec::person_values(&d.by);
            ([id, art, abzug], int(d.at.millis()))
        }
        None => ([Value::Null, Value::Null, Value::Null], Value::Null),
    };
    let [id, art, abzug] = by;
    vec![
        id_value(task),
        text(&i.key),
        text(&i.title),
        text(i.rank.as_str()),
        id,
        art,
        abzug,
        at,
    ]
}

pub(crate) fn check_title(title: &str) -> TaskResult<()> {
    if title.trim().is_empty() {
        return Err(bad("checklist", "empty-title"));
    }
    Ok(())
}

/// The items of a draft: the titles in this order, all open (`Invalid` for an empty title or too many).
pub async fn create_items(c: &v5::Conn<'_>, task: ObjectId, titles: &[String]) -> TaskResult<()> {
    if titles.len() > CHECKLIST_MAX {
        return Err(bad("checklist", "too-many"));
    }
    titles.iter().try_for_each(|t| check_title(t))?;
    for (n, (title, rank)) in titles.iter().zip(Rank::spread(titles.len())).enumerate() {
        let item = ChecklistItem {
            key: format!("k{}", n + 1),
            title: title.clone(),
            done: None,
            rank,
        };
        c.insert(&CHECKLIST, row(task, &item))
            .await
            .map_err(adapt)?;
    }
    if !titles.is_empty() {
        c.insert(
            &CHECKLIST_SEQ,
            vec![
                id_value(task),
                int(i64::try_from(titles.len()).unwrap_or(0)),
            ],
        )
        .await
        .map_err(adapt)?;
    }
    Ok(())
}

/// The key of the next item: one more than the highest number ever given in this task (the counter row), or than the
/// items there are (a task from before the counter), and the counter moves on.
pub(crate) async fn next_key(
    c: &v5::Conn<'_>,
    task: ObjectId,
    items: &[ChecklistItem],
) -> TaskResult<String> {
    let rows = c
        .query(
            "SELECT last FROM tasks_checklist_seq WHERE task = ?",
            vec![id_value(task)],
        )
        .await
        .map_err(adapt)?;
    let seen = match rows.first() {
        Some(r) => v5::codec::get_int(r, 0).map_err(adapt)?,
        None => 0,
    };
    let held = items
        .iter()
        .filter_map(|i| i.key.strip_prefix('k').and_then(|n| n.parse::<i64>().ok()))
        .max()
        .unwrap_or(0);
    let next = seen.max(held) + 1;
    c.exec(
        "DELETE FROM tasks_checklist_seq WHERE task = ?",
        vec![id_value(task)],
    )
    .await
    .map_err(adapt)?;
    c.insert(&CHECKLIST_SEQ, vec![id_value(task), int(next)])
        .await
        .map_err(adapt)?;
    Ok(format!("k{next}"))
}
