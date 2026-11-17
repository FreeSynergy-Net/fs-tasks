//! Reading: `list` (filters, order, pages).
//!
//! Pattern: Specification (a query is a set of conditions, each one a small function) plus offset pages (the cursor is
//! the offset as text). The lists, the trash view, the progress, the parent, the labels, the day range, the ready filter,
//! the order and the page are decided by the database (`conditions`, `order_sql`, `LIMIT`/`OFFSET`); only the text (it needs
//! the Unicode lower case) and the order by day (it needs the zone of the viewer) are applied to the loaded tasks.

use std::cmp::Reverse;

use chrono::NaiveDate;
use chrono_tz::Tz;
use fs_objects::v5;
use fs_tasks_testdata::tasks_v1::{
    ObjectId, Order, Page, Place, TaskQuery, TaskResult, TaskSummary,
};

use crate::access::visible;
use crate::conditions;
use crate::error::bad;
use crate::stored::{Stored, load, load_with};
use crate::summary::{counts_of, readiness_of, summary};
use crate::who::Who;
use crate::zones::{day_in, instant, tz};

/// What only the memory decides: the text (a Unicode lower case, as the trait says). Everything else is in the `SELECT`
/// (`conditions`).
fn matches(s: &Stored, q: &TaskQuery) -> bool {
    let t = &s.task;
    q.text.as_ref().is_none_or(|x| {
        let x = x.to_lowercase();
        t.title.to_lowercase().contains(&x)
            || t.description.to_lowercase().contains(&x)
            || t.checklist
                .iter()
                .any(|i| i.title.to_lowercase().contains(&x))
    })
}

fn sort(rows: &mut [&Stored], order: Order, view: Tz) {
    match order {
        Order::Rank => rows.sort_by_key(|s| (s.task.rank.clone(), s.task.id)),
        Order::Priority => rows.sort_by_key(|s| {
            let p = s.task.priority;
            (p == 0, p, s.task.id)
        }),
        Order::Updated => rows.sort_by_key(|s| (Reverse(s.task.updated), s.task.id)),
        Order::Finished => rows.sort_by_key(|s| {
            let at = s.task.progress_updated;
            (at.is_none(), Reverse(at), s.task.id)
        }),
        Order::Due => rows.sort_by_key(|s| {
            let t = &s.task;
            match t.due.as_ref().or(t.start.as_ref()) {
                None => (true, NaiveDate::MAX, true, 0, t.rank.clone(), t.id),
                Some(d) => {
                    let day = day_in(d, view).unwrap_or(NaiveDate::MAX);
                    let (floating, at) = instant(d).map_or((true, 0), |x| (false, x));
                    (false, day, floating, at, t.rank.clone(), t.id)
                }
            }
        }),
    }
}

pub fn offset(after: Option<&Place>) -> TaskResult<usize> {
    after.map_or(Ok(0), |p| {
        p.0.parse::<usize>().map_err(|_| bad("after", "cursor"))
    })
}

/// One page of `items`; `next` is set while more follow (and for `size = 0`, an empty page with a cursor).
fn page<T>(items: Vec<T>, after: Option<&Place>, size: usize) -> TaskResult<Page<T>> {
    let from = offset(after)?;
    let total = items.len();
    let taken: Vec<T> = items.into_iter().skip(from).take(size).collect();
    let end = from + taken.len();
    let more = size == 0 || end < total;
    Ok(Page {
        items: taken,
        next: more.then(|| Place(end.to_string())),
    })
}

/// The most rows a page holds: a larger `size` is cut to it (the cursor says where the next page starts).
pub const PAGE_MAX: usize = 1000;

/// A task that is not in the trash: not deleted on its own, not with a parent that is, its list not deleted.
pub const ALIVE: &str =
    "o.deleted IS NULL AND (t.deleted_with IS NULL OR wo.deleted IS NULL) AND lo.deleted IS NULL";

/// The order the database can give (`None` for `Due`: it needs the day in the view zone).
fn order_sql(order: Order) -> Option<&'static str> {
    match order {
        Order::Rank => Some("t.rank, t.id"),
        Order::Priority => Some("t.priority = 0, t.priority, t.id"),
        Order::Updated => Some("o.updated DESC, t.id"),
        Order::Finished => Some("t.progress_updated IS NULL, t.progress_updated DESC, t.id"),
        Order::Due => None,
    }
}

/// `list`: the filters, the order and the page are the database's work where the answer is the same as in memory
/// (everything but the text, the day range and the order by day, which are decided on the few rows that are left).
pub async fn list(
    c: &v5::Conn<'_>,
    who: &Who,
    q: &TaskQuery,
    after: Option<&Place>,
    size: usize,
) -> TaskResult<Page<TaskSummary>> {
    let size = size.min(PAGE_MAX);
    let view = tz(&q.view_zone, "view_zone")?;
    let seen: Vec<ObjectId> = visible(c, who).await?.into_iter().map(|l| l.0).collect();
    let lists: Vec<ObjectId> = if q.lists.is_empty() {
        seen
    } else {
        q.lists
            .iter()
            .copied()
            .filter(|l| seen.contains(l))
            .collect()
    };
    let none = || page(Vec::<TaskSummary>::new(), after, size);
    let deps = q.ready.is_some() && any_dependency(c).await?;
    let (Some((cond, params)), false) =
        (conditions::build(q, &lists, view, deps), lists.is_empty())
    else {
        return none();
    };
    let from = offset(after)?;
    let by_db = order_sql(q.order).filter(|_| q.text.is_none());
    let (rows, more) = if let Some(order) = by_db {
        let tail = format!(
            "ORDER BY {order} LIMIT {} OFFSET {from}",
            size.saturating_add(1)
        );
        let mut rows = load_with(c, &cond, params, &tail).await?;
        let more = size == 0 || rows.len() > size;
        rows.truncate(size);
        (rows, more)
    } else {
        let all = load(c, &cond, params).await?;
        let mut kept: Vec<&Stored> = all.iter().filter(|s| matches(s, q)).collect();
        sort(&mut kept, q.order, view);
        let more = size == 0 || from.saturating_add(size) < kept.len();
        let rows = kept.into_iter().skip(from).take(size).cloned().collect();
        (rows, more)
    };
    let counts = counts_of(c, &rows.iter().map(|s| s.task.id).collect::<Vec<_>>()).await?;
    let ready = readiness_of(c, &rows).await?;
    let end = from + rows.len();
    Ok(Page {
        items: rows.iter().map(|s| summary(s, &counts, &ready)).collect(),
        next: more.then(|| Place(end.to_string())),
    })
}

/// Whether any living task holds a dependency (without one the ready filter needs no graph).
async fn any_dependency(c: &v5::Conn<'_>) -> TaskResult<bool> {
    let rows = c
        .query("SELECT 1 FROM tasks_dependency LIMIT 1", Vec::new())
        .await
        .map_err(crate::error::adapt)?;
    Ok(!rows.is_empty())
}
