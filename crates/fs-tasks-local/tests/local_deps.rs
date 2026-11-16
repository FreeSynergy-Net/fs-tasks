//! Dependencies in `LocalTasks`: the rows (typed, in the order given, gone with their task) and the cycle check of `relate`
//! (through three tasks and through a time kind, with the path in the error).
//!
//! Pattern: Behaviour tests (one scenario each, through the trait; the rows are counted with SQL).

#[allow(dead_code)]
mod common;

use common::deps::{dep, fresh, make, tie};
use common::{LocalFx, count};
use fs_tasks_testdata::tasks_v1::{
    Dependency, DependencyKind, Error, Progress, SeriesScope, TaskError, Tasks,
};

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
