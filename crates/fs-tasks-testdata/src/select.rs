//! Reading: `list` (filters, order, pages), `labels`, `lists`, `history`, `changes`, `readiness`.
//!
//! Pattern: Specification (a query is a set of conditions, each one a small function) plus offset pages (the cursor
//! is the offset as text).

use crate::clock::{day_in, instant, tz};
use crate::ids::invalid;
use crate::mutation::Mutation;
use crate::series::{Cur, current};
use crate::state::State;
use crate::tasks_v1::{
    AssigneeFilter, Change, DateField, Due, HistoryEntry, LabelCount, ObjectId, Order, Page,
    ParentFilter, Place, Readiness, TaskList, TaskQuery, TaskResult, TaskSummary, Trash,
    not_possible,
};
use chrono::NaiveDate;
use chrono_tz::Tz;
use std::cmp::Reverse;
use std::collections::BTreeMap;

fn wanted(trash: Trash, deleted: bool) -> bool {
    match trash {
        Trash::Exclude => !deleted,
        Trash::Include => true,
        Trash::Only => deleted,
    }
}

fn offset(after: Option<&Place>) -> TaskResult<usize> {
    after.map_or(Ok(0), |p| {
        p.0.parse::<usize>().map_err(|_| invalid("after", "cursor"))
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

fn days(cur: &Cur, field: DateField, view: Tz) -> Vec<NaiveDate> {
    let day = |d: &Option<Due>| d.as_ref().and_then(|d| day_in(d, view));
    match field {
        DateField::Due => day(&cur.due).into_iter().collect(),
        DateField::Start => day(&cur.start).into_iter().collect(),
        DateField::DueOrStart => day(&cur.due)
            .into_iter()
            .chain(day(&cur.start))
            .min()
            .into_iter()
            .collect(),
    }
}

impl State {
    fn matches(&self, i: usize, q: &TaskQuery, view: Tz) -> bool {
        let t = &self.rows[i].task;
        let cur = current(t);
        let in_range = q.due.as_ref().is_none_or(|r| {
            days(&cur, r.field, view).iter().any(|d| {
                r.from.is_none_or(|f| *d >= f.as_naive()) && r.to.is_none_or(|e| *d <= e.as_naive())
            })
        });
        let text = q.text.as_ref().is_none_or(|x| {
            let x = x.to_lowercase();
            t.title.to_lowercase().contains(&x)
                || t.description.to_lowercase().contains(&x)
                || t.checklist
                    .iter()
                    .any(|c| c.title.to_lowercase().contains(&x))
        });
        let who = match &q.assignee {
            None => true,
            Some(AssigneeFilter::Nobody) => cur.assignees.is_empty(),
            Some(AssigneeFilter::Me) => cur.assignees.iter().any(|a| self.is_me(&a.who)),
            Some(AssigneeFilter::Person(p)) => cur.assignees.iter().any(|a| &a.who == p),
            Some(AssigneeFilter::MyGroups) => {
                cur.assignees.iter().any(|a| self.in_my_groups(&a.who))
            }
        };
        let undated = q
            .undated
            .is_none_or(|u| u == (cur.due.is_none() && cur.start.is_none()));
        let parent = match q.parent {
            None => true,
            Some(ParentFilter::TopLevel) => t.parent.is_none(),
            Some(ParentFilter::Of(p)) => t.parent == Some(p),
        };
        self.sees_task(i)
            && wanted(q.trash, self.trash_of(i).is_some())
            && (q.lists.is_empty() || q.lists.contains(&t.list))
            && (q.progress.is_empty() || q.progress.contains(&cur.progress))
            && in_range
            && text
            && who
            && undated
            && parent
            && q.labels.iter().all(|l| t.labels.contains(l))
            && q.ready
                .is_none_or(|r| r == (self.readiness_of(i) == Readiness::Ready))
    }

    fn sorted(&self, mut rows: Vec<usize>, order: Order, view: Tz) -> Vec<usize> {
        let id = |i: usize| self.rows[i].task.id;
        let rank = |i: usize| self.rows[i].task.rank.clone();
        match order {
            Order::Rank => rows.sort_by_key(|&i| (rank(i), id(i))),
            Order::Priority => rows.sort_by_key(|&i| {
                let p = self.rows[i].task.priority;
                (p == 0, p, id(i))
            }),
            Order::Updated => rows.sort_by_key(|&i| (Reverse(self.rows[i].task.updated), id(i))),
            Order::Finished => rows.sort_by_key(|&i| {
                let at = self.rows[i].task.progress_updated;
                (at.is_none(), Reverse(at), id(i))
            }),
            Order::Due => rows.sort_by_key(|&i| {
                let cur = current(&self.rows[i].task);
                match cur.due.as_ref().or(cur.start.as_ref()) {
                    None => (true, NaiveDate::MAX, true, 0, rank(i), id(i)),
                    Some(d) => {
                        let day = day_in(d, view).unwrap_or(NaiveDate::MAX);
                        let (floating, at) = instant(d).map_or((true, 0), |x| (false, x));
                        (false, day, floating, at, rank(i), id(i))
                    }
                }
            }),
        }
        rows
    }

    /// The rows a query selects, in the order it asks for (all of them; the pages cut from this).
    pub(crate) fn selected(&self, q: &TaskQuery) -> TaskResult<Vec<usize>> {
        if q.ready.is_some() && !self.evaluate {
            return not_possible("dependency.evaluate");
        }
        let view = tz(&q.view_zone)?;
        let rows: Vec<usize> = (0..self.rows.len())
            .filter(|&i| self.matches(i, q, view))
            .collect();
        Ok(self.sorted(rows, q.order, view))
    }

    pub(crate) fn query(
        &self,
        q: &TaskQuery,
        after: Option<&Place>,
        size: usize,
    ) -> TaskResult<Page<TaskSummary>> {
        let summaries = self
            .selected(q)?
            .into_iter()
            .map(|i| self.summary(i))
            .collect();
        page(summaries, after, size)
    }

    /// The lists the asking person may see, with their access and trash state.
    pub(crate) fn lists_of(&self, trash: Trash) -> Vec<TaskList> {
        (0..self.lists.len())
            .filter(|&i| {
                self.sees(&self.lists[i])
                    && wanted(trash, self.list_bins.contains_key(&self.lists[i].id))
            })
            .map(|i| self.shown(i))
            .collect()
    }

    pub(crate) fn label_counts(
        &self,
        lists: &[ObjectId],
        prefix: Option<&str>,
        after: Option<&Place>,
        size: usize,
    ) -> TaskResult<Page<LabelCount>> {
        let mut counts = BTreeMap::<String, u32>::new();
        for i in (0..self.rows.len()).filter(|&i| self.trash_of(i).is_none() && self.sees_task(i)) {
            let t = &self.rows[i].task;
            if !lists.is_empty() && !lists.contains(&t.list) {
                continue;
            }
            for l in t
                .labels
                .iter()
                .filter(|l| prefix.is_none_or(|p| l.starts_with(p)))
            {
                *counts.entry(l.clone()).or_default() += 1;
            }
        }
        let all = counts
            .into_iter()
            .map(|(label, count)| LabelCount { label, count })
            .collect();
        page(all, after, size)
    }

    pub(crate) fn readiness_many(
        &self,
        ids: &[ObjectId],
    ) -> TaskResult<Vec<(ObjectId, Readiness)>> {
        if !self.evaluate {
            return not_possible("dependency.evaluate");
        }
        ids.iter()
            .map(|&id| Ok((id, self.readiness_of(self.live(id)?))))
            .collect()
    }

    pub(crate) fn log_of(
        &self,
        id: ObjectId,
        after: Option<&Place>,
        size: usize,
    ) -> TaskResult<Page<HistoryEntry>> {
        // Reading a list covers its log: whoever may not see the list gets `NotFound`, as for `get`.
        self.at(id)
            .filter(|&i| self.sees_task(i) || self.mutated(Mutation::LeakTrash))
            .ok_or(crate::tasks_v1::Error::NotFound(id))?;
        let entries = self
            .history
            .iter()
            .filter(|(t, _)| *t == id)
            .map(|(_, e)| e.clone())
            .collect();
        page(entries, after, size)
    }

    /// The change feed: `None` = from now on; `next` is ALWAYS set, so "from now on" can ask again.
    pub(crate) fn feed_since(
        &self,
        since: Option<&Place>,
        size: usize,
    ) -> TaskResult<Page<Change>> {
        let from = match since {
            None => self.changes.len(),
            Some(_) => offset(since)?,
        };
        if from > self.changes.len() {
            return Err(invalid("since", "cursor"));
        }
        // What the asker may not see is left out, and the page is filled from what follows: a page is empty only at
        // the end of the feed (the cursor counts what was read, not what was shown).
        let mut items: Vec<Change> = Vec::new();
        let mut at = from;
        while at < self.changes.len() && items.len() < size {
            let c = &self.changes[at];
            if self.sees_feed_of(c.list) || self.mutated(Mutation::LeakTrash) {
                items.push(c.clone());
            }
            at += 1;
        }
        Ok(Page {
            items,
            next: Some(Place(at.to_string())),
        })
    }
}
