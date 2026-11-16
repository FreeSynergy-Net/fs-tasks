//! Dependencies in `LocalTasks`: the rows, the cycle check of `relate`, and the ready filter of `list` (SQL) held against the
//! judgement of `readiness` (Rust) over a graph that has every state: blocked, ready, cancelled, unknown, a cycle that came
//! in through a restore, the four time kinds with a started and a not started predecessor.
//!
//! Pattern: Agreement test (two implementations of one rule answer the same on every task of one fixed graph).

#[allow(dead_code)]
mod common;

use common::{LocalFx, count, draft};
use fs_tasks_testdata::tasks_v1::{
    Dependency, DependencyKind, Error, ObjectId, Progress, Readiness, SeriesScope, Task, TaskError,
    TaskPatch, TaskQuery, Tasks, Trash,
};

fn dep(other: &Task, kind: DependencyKind) -> Dependency {
    Dependency {
        other_uid: other.uid.clone(),
        kind,
        gap: None,
    }
}

async fn make(fx: &LocalFx, list: ObjectId, title: &str, to: Progress) -> Task {
    let t = fx
        .tasks
        .create(draft(list, title, TaskPatch::default()))
        .await
        .unwrap();
    if to == Progress::NeedsAction {
        return t;
    }
    fx.tasks
        .progress(
            t.id,
            t.revision,
            to,
            fs_tasks_testdata::tasks_v1::Children::Leave,
            SeriesScope::Whole,
        )
        .await
        .unwrap()
}

async fn fresh(fx: &LocalFx, t: &Task) -> Task {
    fx.tasks.get(t.id).await.unwrap().unwrap()
}

/// `holder` holds `kind` to `other` (the holder of a time kind is the predecessor; of `DependsOn` the waiting task).
async fn tie(fx: &LocalFx, holder: &Task, other: &Task, kind: DependencyKind) {
    let h = fresh(fx, holder).await;
    fx.tasks
        .relate(h.id, h.revision, dep(other, kind))
        .await
        .unwrap();
}

fn query(list: ObjectId) -> TaskQuery {
    use fs_tasks_testdata::tasks_v1::{Order, Zone};
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

async fn ids(fx: &LocalFx, q: &TaskQuery) -> Vec<ObjectId> {
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

#[tokio::test]
async fn the_ready_filter_in_sql_agrees_with_the_readiness_of_every_task() {
    use DependencyKind::{DependsOn, FinishToFinish, FinishToStart, StartToFinish, StartToStart};
    use Progress::{Cancelled, Completed, Failed, InProcess, NeedsAction};
    let fx = LocalFx::memory().await;
    let list = fx.tasks.create_list("L", None, None).await.unwrap().id;
    let open = make(&fx, list, "open", NeedsAction).await;
    let started = make(&fx, list, "started", InProcess).await;
    let done = make(&fx, list, "done", Completed).await;
    let cancelled = make(&fx, list, "cancelled", Cancelled).await;
    let failed = make(&fx, list, "failed", Failed).await;
    let mut waiting = Vec::new();
    for (name, on) in [
        ("w-open", &open),
        ("w-started", &started),
        ("w-done", &done),
        ("w-cancelled", &cancelled),
        ("w-failed", &failed),
    ] {
        let w = make(&fx, list, name, NeedsAction).await;
        tie(&fx, &w, on, DependsOn).await;
        waiting.push(w);
    }
    // The holder of a time kind is the predecessor: `y*` wait for `x*`.
    for (kind, name) in [
        (FinishToStart, "f2s"),
        (FinishToFinish, "f2f"),
        (StartToStart, "s2s"),
        (StartToFinish, "s2f"),
    ] {
        for (state, tag) in [(NeedsAction, "open"), (InProcess, "started")] {
            let x = make(&fx, list, &format!("x-{name}-{tag}"), state).await;
            let y = make(&fx, list, &format!("y-{name}-{tag}"), NeedsAction).await;
            tie(&fx, &x, &y, kind).await;
        }
    }
    let unknown = make(&fx, list, "unknown", NeedsAction).await;
    fx.tasks
        .relate(
            unknown.id,
            unknown.revision,
            Dependency {
                other_uid: "urn:uuid:nobody".into(),
                kind: DependsOn,
                gap: None,
            },
        )
        .await
        .unwrap();
    let finished_with_deps = make(&fx, list, "finished", Completed).await;
    tie(&fx, &finished_with_deps, &open, DependsOn).await;
    // A cycle that comes in through a restore: `relate` cannot see a tie held by a task in the trash.
    let c1 = make(&fx, list, "c1", NeedsAction).await;
    let c2 = make(&fx, list, "c2", NeedsAction).await;
    tie(&fx, &c2, &c1, DependsOn).await;
    let c2 = fresh(&fx, &c2).await;
    fx.tasks
        .delete(c2.id, c2.revision, SeriesScope::Whole)
        .await
        .unwrap();
    tie(&fx, &c1, &c2, DependsOn).await;
    fx.tasks.restore(c2.id, SeriesScope::Whole).await.unwrap();
    // A task that waits for a task in the trash: the name is no living task, so it is unknown.
    let gone = make(&fx, list, "gone", NeedsAction).await;
    let w_gone = make(&fx, list, "w-gone", NeedsAction).await;
    tie(&fx, &w_gone, &gone, DependsOn).await;
    let gone = fresh(&fx, &gone).await;
    fx.tasks
        .delete(gone.id, gone.revision, SeriesScope::Whole)
        .await
        .unwrap();

    let all = ids(&fx, &query(list)).await;
    let asked = fx.tasks.readiness(&all).await.unwrap();
    assert_eq!(asked.iter().map(|a| a.0).collect::<Vec<_>>(), all);
    let ready: Vec<ObjectId> = {
        let mut r: Vec<ObjectId> = asked
            .iter()
            .filter(|a| a.1 == Readiness::Ready)
            .map(|a| a.0)
            .collect();
        r.sort();
        r
    };
    let mut q = query(list);
    q.ready = Some(true);
    assert_eq!(ids(&fx, &q).await, ready, "ready = Some(true)");
    q.ready = Some(false);
    let not_ready: Vec<ObjectId> = all.iter().copied().filter(|i| !ready.contains(i)).collect();
    assert_eq!(ids(&fx, &q).await, not_ready, "ready = Some(false)");
    // The summaries say the same as `readiness`.
    let page = fx.tasks.list(&query(list), None, 1000).await.unwrap();
    for s in page.items {
        let want = asked.iter().find(|a| a.0 == s.id).unwrap();
        assert_eq!(s.readiness, want.1, "the card of {}", s.title);
    }
    // And the values are the ones the rules of the trait name.
    let of = |t: &Task| asked.iter().find(|a| a.0 == t.id).unwrap().1.clone();
    assert_eq!(of(&waiting[0]), Readiness::Blocked(vec![open.uid.clone()]));
    assert_eq!(
        of(&waiting[1]),
        Readiness::Blocked(vec![started.uid.clone()])
    );
    assert_eq!(of(&waiting[2]), Readiness::Ready);
    assert_eq!(
        of(&waiting[3]),
        Readiness::DependencyCancelled(vec![cancelled.uid.clone()])
    );
    assert_eq!(
        of(&waiting[4]),
        Readiness::DependencyCancelled(vec![failed.uid.clone()])
    );
    assert_eq!(
        of(&unknown),
        Readiness::Unknown(vec!["urn:uuid:nobody".into()])
    );
    assert_eq!(of(&finished_with_deps), Readiness::Done);
    let mut cycle = vec![c1.uid.clone(), c2.uid.clone()];
    cycle.sort();
    assert_eq!(of(&c1), Readiness::Conflict(cycle.clone()));
    assert_eq!(of(&c2), Readiness::Conflict(cycle));
    assert_eq!(of(&w_gone), Readiness::Unknown(vec![gone.uid.clone()]));
    for name in ["y-s2s-started", "y-s2f-started", "y-f2s-open", "y-s2s-open"] {
        let t = all_by_title(&fx, list, name).await;
        let started_enough = name.contains("s2") && name.ends_with("started");
        let want = if started_enough {
            Readiness::Ready
        } else {
            let x = all_by_title(&fx, list, &name.replace("y-", "x-")).await;
            Readiness::Blocked(vec![x.uid])
        };
        assert_eq!(of(&t), want, "{name}");
    }
    let t = all_by_title(&fx, list, "y-f2f-started").await;
    assert!(
        matches!(of(&t), Readiness::Blocked(_)),
        "finish needs completion"
    );
}

async fn all_by_title(fx: &LocalFx, list: ObjectId, title: &str) -> Task {
    for s in fx.tasks.list(&query(list), None, 1000).await.unwrap().items {
        if s.title == title {
            return fx.tasks.get(s.id).await.unwrap().unwrap();
        }
    }
    panic!("no task {title}");
}

#[tokio::test]
async fn a_dependency_is_a_typed_row_in_order_and_goes_with_its_task() {
    let fx = LocalFx::memory().await;
    let list = fx.tasks.create_list("L", None, None).await.unwrap().id;
    let a = make(&fx, list, "a", Progress::NeedsAction).await;
    let b = make(&fx, list, "b", Progress::NeedsAction).await;
    let c = make(&fx, list, "c", Progress::NeedsAction).await;
    let gap = fs_tasks_testdata::tasks_v1::Gap {
        negative: true,
        span: fs_tasks_testdata::tasks_v1::Span {
            days: 2,
            seconds: 30,
        },
    };
    let a1 = fx
        .tasks
        .relate(
            a.id,
            a.revision,
            Dependency {
                gap: Some(gap),
                ..dep(&b, DependencyKind::FinishToStart)
            },
        )
        .await
        .unwrap();
    let a2 = fx
        .tasks
        .relate(a1.id, a1.revision, dep(&c, DependencyKind::DependsOn))
        .await
        .unwrap();
    assert_eq!(a2.dependencies[0].gap, Some(gap));
    assert_eq!(a2.dependencies[1].other_uid, c.uid);
    // The same tie twice: no change, no new revision.
    let again = fx
        .tasks
        .relate(a2.id, a2.revision, dep(&c, DependencyKind::DependsOn))
        .await
        .unwrap();
    assert_eq!(again.revision, a2.revision);
    assert_eq!(count(&fx, "tasks_dependency").await, 2);
    // A removed tie goes, the order of the rest holds, a later one comes after.
    let a3 = fx
        .tasks
        .unrelate(a2.id, a2.revision, &b.uid, DependencyKind::FinishToStart)
        .await
        .unwrap();
    let a4 = fx
        .tasks
        .relate(a3.id, a3.revision, dep(&b, DependencyKind::DependsOn))
        .await
        .unwrap();
    let others: Vec<&str> = a4
        .dependencies
        .iter()
        .map(|d| d.other_uid.as_str())
        .collect();
    assert_eq!(others, [c.uid.as_str(), b.uid.as_str()]);
    // The final delete takes the rows with the task.
    fx.tasks
        .delete(a4.id, a4.revision, SeriesScope::Whole)
        .await
        .unwrap();
    fx.tasks.purge(a4.id).await.unwrap();
    assert_eq!(count(&fx, "tasks_dependency").await, 0);
}

#[tokio::test]
async fn a_cycle_through_three_tasks_and_through_a_time_kind_is_refused_with_its_path() {
    let fx = LocalFx::memory().await;
    let list = fx.tasks.create_list("L", None, None).await.unwrap().id;
    let a = make(&fx, list, "a", Progress::NeedsAction).await;
    let b = make(&fx, list, "b", Progress::NeedsAction).await;
    let c = make(&fx, list, "c", Progress::NeedsAction).await;
    tie(&fx, &a, &b, DependencyKind::DependsOn).await;
    tie(&fx, &b, &c, DependencyKind::DependsOn).await;
    let c = fresh(&fx, &c).await;
    let r = fx
        .tasks
        .relate(c.id, c.revision, dep(&a, DependencyKind::DependsOn))
        .await;
    let Err(TaskError::Cycle(path)) = r else {
        panic!("a cycle: {r:?}");
    };
    for t in [&a, &b, &c] {
        assert!(path.contains(&t.uid), "{path:?}");
    }
    // `a` waits for `b`, `b` for `c`: `c` holding finish-to-start toward `a` makes `a` its successor: `a` waits for `c`, no
    // cycle; `a` holding it toward `c` makes `c` wait for `a`: a cycle.
    let a = fresh(&fx, &a).await;
    let r = fx
        .tasks
        .relate(a.id, a.revision, dep(&c, DependencyKind::FinishToStart))
        .await;
    assert!(matches!(r, Err(TaskError::Cycle(_))), "{r:?}");
    let c = fresh(&fx, &c).await;
    let r = fx
        .tasks
        .relate(c.id, c.revision, dep(&a, DependencyKind::FinishToStart))
        .await;
    assert!(r.is_ok(), "{r:?}");
    // The refused ties are not stored.
    assert_eq!(count(&fx, "tasks_dependency").await, 3);
    let r = fx
        .tasks
        .readiness(&[fs_tasks_testdata::object_id(u64::MAX)])
        .await;
    assert!(matches!(r, Err(TaskError::Common(Error::NotFound(_)))));
}
