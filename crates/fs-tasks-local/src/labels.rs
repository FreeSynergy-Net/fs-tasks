//! `labels`: the labels of the living tasks of the lists the asker sees, counted per task, alphabetical, by page.
//!
//! Pattern: Query function (one grouped `SELECT`; the database counts, sorts and pages).

use fs_objects::v5;
use fs_objects::v5::codec::{get_int, get_text, id_value, text};
use fs_tasks_testdata::tasks_v1::{LabelCount, ObjectId, Page, Place, TaskResult};
use turso::Value;

use crate::access::visible;
use crate::error::adapt;
use crate::query::{ALIVE, PAGE_MAX, offset};
use crate::stored::marks;
use crate::who::Who;

/// `labels`: a label counts once per task; `prefix` is case-sensitive; the cursor is the offset as text.
pub async fn labels(
    c: &v5::Conn<'_>,
    who: &Who,
    lists: &[ObjectId],
    prefix: Option<&str>,
    after: Option<&Place>,
    size: usize,
) -> TaskResult<Page<LabelCount>> {
    let size = size.min(PAGE_MAX);
    let from = offset(after)?;
    let seen: Vec<ObjectId> = visible(c, who).await?.into_iter().map(|l| l.0).collect();
    let lists: Vec<ObjectId> = if lists.is_empty() {
        seen
    } else {
        lists.iter().copied().filter(|l| seen.contains(l)).collect()
    };
    let empty = Page {
        items: Vec::new(),
        next: (size == 0).then(|| Place(from.to_string())),
    };
    if lists.is_empty() {
        return Ok(empty);
    }
    let mut params: Vec<Value> = lists.iter().map(|l| id_value(*l)).collect();
    let mut cond = format!("t.list IN ({}) AND {ALIVE}", marks(lists.len()));
    if let Some(p) = prefix {
        cond.push_str(" AND substr(lb.value, 1, ?) = ?");
        params.push(v5::codec::int(
            i64::try_from(p.chars().count()).unwrap_or(0),
        ));
        params.push(text(p));
    }
    let sql = format!(
        "SELECT lb.value, count(*) FROM tasks_label lb JOIN tasks_task t ON t.id = lb.task \
         JOIN objects o ON o.id = t.id JOIN objects lo ON lo.id = t.list \
         LEFT JOIN objects wo ON wo.id = t.deleted_with WHERE {cond} \
         GROUP BY lb.value ORDER BY lb.value LIMIT {} OFFSET {from}",
        size.saturating_add(1)
    );
    let rows = c.query(&sql, params).await.map_err(adapt)?;
    let more = size == 0 || rows.len() > size;
    let mut items = Vec::new();
    for r in rows.iter().take(size) {
        items.push(LabelCount {
            label: get_text(r, 0).map_err(adapt)?,
            count: u32::try_from(get_int(r, 1).map_err(adapt)?).unwrap_or(u32::MAX),
        });
    }
    let end = from + items.len();
    Ok(Page {
        items,
        next: more.then(|| Place(end.to_string())),
    })
}
