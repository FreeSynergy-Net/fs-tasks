//! The ready filter of `list` (one SQL condition with a recursive query) held against the judgement of `readiness` (Rust over
//! the graph of the living tasks) on a graph that has every state: blocked, ready, cancelled, failed, unknown, a cycle that came
//! in through a restore (also one with a completed member), a predecessor in the trash, the four time kinds with a started and
//! a not started predecessor. The cards say the same as `readiness`.
//!
//! Pattern: Agreement test (two implementations of one rule answer the same on every task of one fixed graph).

#[allow(dead_code)]
mod common;

use DependencyKind::{DependsOn, FinishToFinish, FinishToStart, StartToFinish, StartToStart};
use Progress::{Cancelled, Completed, Failed, InProcess, NeedsAction};
use common::LocalFx;
use common::deps::{by_title, close_cycle, fresh, ids, list_query, make, tie};
use fs_tasks_testdata::tasks_v1::{
    Children, Dependency, DependencyKind, ObjectId, Progress, Readiness, SeriesScope, Task, Tasks,
};

/// The named tasks of the graph.
struct World {
    list: ObjectId,
    open: Task,
    started: Task,
    cancelled: Task,
    failed: Task,
    waiting: Vec<Task>,
    unknown: Task,
    finished: Task,
    cycle: [Task; 2],
    closed: [Task; 2],
    on_gone: Task,
    gone: Task,
}

/// Waiting on a task in each state, the four time kinds, an unknown name, a finished task with a tie.
async fn chains(fx: &LocalFx, list: ObjectId) -> (Task, Task, Task, Task, Vec<Task>, Task, Task) {
    let open = make(fx, list, "open", NeedsAction).await;
    let started = make(fx, list, "started", InProcess).await;
    let done = make(fx, list, "done", Completed).await;
    let cancelled = make(fx, list, "cancelled", Cancelled).await;
    let failed = make(fx, list, "failed", Failed).await;
    let mut waiting = Vec::new();
    for (name, on) in [
        ("w-open", &open),
        ("w-started", &started),
        ("w-done", &done),
        ("w-cancelled", &cancelled),
        ("w-failed", &failed),
    ] {
        let w = make(fx, list, name, NeedsAction).await;
        tie(fx, &w, on, DependsOn).await;
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
            let x = make(fx, list, &format!("x-{name}-{tag}"), state).await;
            let y = make(fx, list, &format!("y-{name}-{tag}"), NeedsAction).await;
            tie(fx, &x, &y, kind).await;
        }
    }
    let unknown = make(fx, list, "unknown", NeedsAction).await;
    let nobody = Dependency {
        other_uid: "urn:uuid:nobody".into(),
        kind: DependsOn,
        gap: None,
    };
    fx.tasks
        .relate(unknown.id, unknown.revision, nobody)
        .await
        .unwrap();
    let finished = make(fx, list, "finished", Completed).await;
    tie(fx, &finished, &open, DependsOn).await;
    (open, started, cancelled, failed, waiting, unknown, finished)
}

async fn world(fx: &LocalFx) -> World {
    let list = fx.tasks.create_list("L", None, None).await.unwrap().id;
    let (open, started, cancelled, failed, waiting, unknown, finished) = chains(fx, list).await;
    let c1 = make(fx, list, "c1", NeedsAction).await;
    let c2 = make(fx, list, "c2", NeedsAction).await;
    let c2 = close_cycle(fx, &c1, &c2).await;
    // The same cycle with one member completed: the open one is no longer in its own way, only the cycle says `Conflict`.
    let c3 = make(fx, list, "c3", NeedsAction).await;
    let c4 = make(fx, list, "c4", NeedsAction).await;
    let c4 = close_cycle(fx, &c3, &c4).await;
    let c4 = fx
        .tasks
        .progress(
            c4.id,
            c4.revision,
            Completed,
            Children::Leave,
            SeriesScope::Whole,
        )
        .await
        .unwrap();
    // A task that waits for a task in the trash: the name is no living task, so it is unknown.
    let gone = make(fx, list, "gone", NeedsAction).await;
    let on_gone = make(fx, list, "on-gone", NeedsAction).await;
    tie(fx, &on_gone, &gone, DependsOn).await;
    let gone = fresh(fx, &gone).await;
    fx.tasks
        .delete(gone.id, gone.revision, SeriesScope::Whole)
        .await
        .unwrap();
    World {
        list,
        open,
        started,
        cancelled,
        failed,
        waiting,
        unknown,
        finished,
        cycle: [fresh(fx, &c1).await, c2],
        closed: [fresh(fx, &c3).await, c4],
        on_gone,
        gone,
    }
}

fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

#[tokio::test]
async fn the_ready_filter_in_sql_agrees_with_the_readiness_of_every_task() {
    let fx = LocalFx::memory().await;
    let w = Box::pin(world(&fx)).await;
    let all = ids(&fx, &list_query(w.list)).await;
    let asked = fx.tasks.readiness(&all).await.unwrap();
    assert_eq!(asked.iter().map(|a| a.0).collect::<Vec<_>>(), all);
    let mut ready: Vec<ObjectId> = asked
        .iter()
        .filter(|a| a.1 == Readiness::Ready)
        .map(|a| a.0)
        .collect();
    ready.sort();
    let mut q = list_query(w.list);
    q.ready = Some(true);
    assert_eq!(ids(&fx, &q).await, ready, "ready = Some(true)");
    q.ready = Some(false);
    let not_ready: Vec<ObjectId> = all.iter().copied().filter(|i| !ready.contains(i)).collect();
    assert_eq!(ids(&fx, &q).await, not_ready, "ready = Some(false)");
    let page = fx
        .tasks
        .list(&list_query(w.list), None, 1000)
        .await
        .unwrap();
    for s in page.items {
        let want = asked.iter().find(|a| a.0 == s.id).unwrap();
        assert_eq!(s.readiness, want.1, "the card of {}", s.title);
    }
}

#[tokio::test]
async fn readiness_says_what_the_rules_of_the_trait_name() {
    let fx = LocalFx::memory().await;
    let w = Box::pin(world(&fx)).await;
    let all = ids(&fx, &list_query(w.list)).await;
    let asked = fx.tasks.readiness(&all).await.unwrap();
    let of = |t: &Task| asked.iter().find(|a| a.0 == t.id).unwrap().1.clone();
    let uid = |t: &Task| vec![t.uid.clone()];
    assert_eq!(of(&w.waiting[0]), Readiness::Blocked(uid(&w.open)));
    assert_eq!(of(&w.waiting[1]), Readiness::Blocked(uid(&w.started)));
    assert_eq!(of(&w.waiting[2]), Readiness::Ready);
    assert_eq!(
        of(&w.waiting[3]),
        Readiness::DependencyCancelled(uid(&w.cancelled))
    );
    assert_eq!(
        of(&w.waiting[4]),
        Readiness::DependencyCancelled(uid(&w.failed))
    );
    assert_eq!(
        of(&w.unknown),
        Readiness::Unknown(vec!["urn:uuid:nobody".into()])
    );
    assert_eq!(of(&w.finished), Readiness::Done);
    let both = |t: &[Task; 2]| sorted(t.iter().map(|x| x.uid.clone()).collect());
    assert_eq!(of(&w.cycle[0]), Readiness::Conflict(both(&w.cycle)));
    assert_eq!(of(&w.cycle[1]), Readiness::Conflict(both(&w.cycle)));
    assert_eq!(of(&w.closed[0]), Readiness::Conflict(both(&w.closed)));
    assert_eq!(of(&w.closed[1]), Readiness::Done);
    assert_eq!(of(&w.on_gone), Readiness::Unknown(uid(&w.gone)));
    for name in ["y-s2s-started", "y-s2f-started", "y-f2s-open", "y-s2s-open"] {
        let t = by_title(&fx, w.list, name).await;
        let started_enough = name.contains("s2") && name.ends_with("started");
        let want = if started_enough {
            Readiness::Ready
        } else {
            let x = by_title(&fx, w.list, &name.replace("y-", "x-")).await;
            Readiness::Blocked(vec![x.uid])
        };
        assert_eq!(of(&t), want, "{name}");
    }
    let t = by_title(&fx, w.list, "y-f2f-started").await;
    assert!(
        matches!(of(&t), Readiness::Blocked(_)),
        "finish needs completion"
    );
}
