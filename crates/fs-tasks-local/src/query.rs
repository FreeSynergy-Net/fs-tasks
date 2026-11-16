//! Reading: `list` (filters, order, pages).
//!
//! Pattern: Specification (a query is a set of conditions, each one a small function) plus offset pages (the cursor is
//! the offset as text). The list filter goes into the `SELECT`; the rest is applied to the loaded tasks (run 2 moves
//! the day ranges and the order into SQL with the derived columns).

use std::cmp::Reverse;

use chrono::NaiveDate;
use chrono_tz::Tz;
use fs_objects::v5;
use fs_objects::v5::codec::id_value;
use fs_tasks_testdata::tasks_v1::{
    AssigneeFilter, DateField, Due, Order, Page, ParentFilter, Place, TaskQuery, TaskResult,
    TaskSummary, Trash, not_possible,
};

use crate::error::bad;
use crate::stored::{Stored, load, marks};
use crate::summary::{counts, summary};
use crate::zones::{day_in, instant, tz};

fn wanted(view: Trash, deleted: bool) -> bool {
    match view {
        Trash::Exclude => !deleted,
        Trash::Include => true,
        Trash::Only => deleted,
    }
}

fn day_of(due: Option<&Due>, view: Tz) -> Option<NaiveDate> {
    due.and_then(|d| day_in(d, view))
}

fn in_days(s: &Stored, q: &TaskQuery, view: Tz) -> bool {
    let Some(r) = &q.due else { return true };
    let t = &s.task;
    let days: Vec<NaiveDate> = match r.field {
        DateField::Due => day_of(t.due.as_ref(), view).into_iter().collect(),
        DateField::Start => day_of(t.start.as_ref(), view).into_iter().collect(),
        DateField::DueOrStart => [day_of(t.due.as_ref(), view), day_of(t.start.as_ref(), view)]
            .into_iter()
            .flatten()
            .min()
            .into_iter()
            .collect(),
    };
    days.iter().any(|d| {
        r.from.is_none_or(|f| *d >= f.as_naive()) && r.to.is_none_or(|e| *d <= e.as_naive())
    })
}

fn matches(s: &Stored, q: &TaskQuery, view: Tz) -> bool {
    let t = &s.task;
    let text = q.text.as_ref().is_none_or(|x| {
        let x = x.to_lowercase();
        t.title.to_lowercase().contains(&x) || t.description.to_lowercase().contains(&x)
    });
    // No assignees are stored in this run: only "nobody" matches.
    let who = matches!(q.assignee, None | Some(AssigneeFilter::Nobody));
    let undated = q
        .undated
        .is_none_or(|u| u == (t.due.is_none() && t.start.is_none()));
    let parent = match q.parent {
        None => true,
        Some(ParentFilter::TopLevel) => t.parent.is_none(),
        Some(ParentFilter::Of(p)) => t.parent == Some(p),
    };
    wanted(q.trash, !s.alive())
        && (q.progress.is_empty() || q.progress.contains(&t.progress))
        && in_days(s, q, view)
        && text
        && who
        && undated
        && parent
        && q.labels.iter().all(|l| t.labels.contains(l))
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

fn offset(after: Option<&Place>) -> TaskResult<usize> {
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

/// `list`.
pub async fn list(
    c: &v5::Conn<'_>,
    q: &TaskQuery,
    after: Option<&Place>,
    size: usize,
) -> TaskResult<Page<TaskSummary>> {
    if q.ready.is_some() {
        return not_possible("dependency.evaluate");
    }
    let view = tz(&q.view_zone, "view_zone")?;
    let (cond, params) = if q.lists.is_empty() {
        ("1 = 1".to_owned(), Vec::new())
    } else {
        (
            format!("t.list IN ({})", marks(q.lists.len())),
            q.lists.iter().map(|l| id_value(*l)).collect(),
        )
    };
    let all = load(c, &cond, params).await?;
    let mut rows: Vec<&Stored> = all.iter().filter(|s| matches(s, q, view)).collect();
    sort(&mut rows, q.order, view);
    let counts = counts(&all);
    page(
        rows.into_iter().map(|s| summary(s, &counts)).collect(),
        after,
        size,
    )
}
