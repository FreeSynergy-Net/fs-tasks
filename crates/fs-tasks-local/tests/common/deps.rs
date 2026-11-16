//! Helpers of the tests of dependencies: make a task in a state, tie two tasks, ask a list, close a cycle through the trash.
//!
//! Pattern: Test helpers (small functions over the provider, through the trait only).

use super::{LocalFx, draft};
use fs_tasks_testdata::tasks_v1::{
    Children, Dependency, DependencyKind, ObjectId, Order, Progress, SeriesScope, Task, TaskPatch,
    TaskQuery, Tasks, Trash, Zone,
};

pub fn dep(other: &Task, kind: DependencyKind) -> Dependency {
    Dependency {
        other_uid: other.uid.clone(),
        kind,
        gap: None,
    }
}

/// A task in `list`, moved to the state `to`.
pub async fn make(fx: &LocalFx, list: ObjectId, title: &str, to: Progress) -> Task {
    let t = fx
        .tasks
        .create(draft(list, title, TaskPatch::default()))
        .await
        .unwrap();
    if to == Progress::NeedsAction {
        return t;
    }
    fx.tasks
        .progress(t.id, t.revision, to, Children::Leave, SeriesScope::Whole)
        .await
        .unwrap()
}

pub async fn fresh(fx: &LocalFx, t: &Task) -> Task {
    fx.tasks.get(t.id).await.unwrap().unwrap()
}

/// `holder` holds `kind` to `other` (the holder of a time kind is the predecessor; of `DependsOn` the waiting task).
pub async fn tie(fx: &LocalFx, holder: &Task, other: &Task, kind: DependencyKind) {
    let h = fresh(fx, holder).await;
    fx.tasks
        .relate(h.id, h.revision, dep(other, kind))
        .await
        .unwrap();
}

/// Everything in `list`, by rank, in UTC.
pub fn list_query(list: ObjectId) -> TaskQuery {
    TaskQuery {
        lists: vec![list],
        text: None,
        progress: Vec::new(),
        due: None,
        undated: None,
        assignee: None,
        labels: Vec::new(),
        parent: None,
        ready: None,
        order: Order::Rank,
        view_zone: Zone::iana("UTC").unwrap(),
        trash: Trash::Exclude,
    }
}

pub async fn ids(fx: &LocalFx, q: &TaskQuery) -> Vec<ObjectId> {
    let mut out: Vec<ObjectId> = fx
        .tasks
        .list(q, None, 1000)
        .await
        .unwrap()
        .items
        .into_iter()
        .map(|s| s.id)
        .collect();
    out.sort();
    out
}

pub async fn by_title(fx: &LocalFx, list: ObjectId, title: &str) -> Task {
    for s in fx
        .tasks
        .list(&list_query(list), None, 1000)
        .await
        .unwrap()
        .items
    {
        if s.title == title {
            return fx.tasks.get(s.id).await.unwrap().unwrap();
        }
    }
    panic!("no task {title}");
}

/// `a` and `b` wait for each other: `relate` cannot see a tie held by a task in the trash, so `b` goes to the trash while
/// `a` is tied to it, and comes back. Returns `b` as it is then.
pub async fn close_cycle(fx: &LocalFx, a: &Task, b: &Task) -> Task {
    tie(fx, b, a, DependencyKind::DependsOn).await;
    let b = fresh(fx, b).await;
    fx.tasks
        .delete(b.id, b.revision, SeriesScope::Whole)
        .await
        .unwrap();
    tie(fx, a, &b, DependencyKind::DependsOn).await;
    fx.tasks.restore(b.id, SeriesScope::Whole).await.unwrap()
}
