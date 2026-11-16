//! The ready filter of `list` as one condition on the task `t`: it is `Readiness::Ready` exactly when it is open, in no
//! cycle, names no unknown task and has no predecessor that is still in the way. The database decides (a recursive query
//! finds the cycles); `graph.rs` makes the same judgement per task for the `readiness` method and the summaries, and the
//! tests hold the two against each other.
//!
//! Pattern: Specification (a condition text built from small named parts).

/// `JOIN`s that give the trash state of a task with the alias `a` (its own, that of the task it went with, its list's).
fn joins(a: &str) -> String {
    format!(
        "JOIN objects {a}o ON {a}o.id = {a}.id JOIN objects {a}lo ON {a}lo.id = {a}.list \
         LEFT JOIN objects {a}wo ON {a}wo.id = {a}.deleted_with"
    )
}

/// The task with the alias `a` is not in the trash.
fn alive(a: &str) -> String {
    format!(
        "{a}o.deleted IS NULL AND ({a}.deleted_with IS NULL OR {a}wo.deleted IS NULL) AND {a}lo.deleted IS NULL"
    )
}

/// Who waits for whom (waiter, predecessor, kind): a task holding `depends-on` waits for the other; the holder of a time
/// kind is the predecessor of the other. Only ties held by living tasks count.
fn waits() -> String {
    let (j, live) = (joins("h"), alive("h"));
    format!(
        "SELECT h.uid AS waiter, d.other_uid AS pred, d.kind AS kind FROM tasks_dependency d \
         JOIN tasks_task h ON h.id = d.task {j} WHERE d.kind = 'depends-on' AND {live} \
         UNION ALL \
         SELECT d.other_uid, h.uid, d.kind FROM tasks_dependency d \
         JOIN tasks_task h ON h.id = d.task {j} WHERE d.kind <> 'depends-on' AND {live}"
    )
}

/// The `uid`s that lie on a cycle of "waits for".
fn on_a_cycle() -> String {
    let w = waits();
    format!(
        "WITH RECURSIVE w(a, b) AS (SELECT waiter, pred FROM ({w})), r(s, x) AS (SELECT a, b FROM w UNION \
         SELECT r.s, w.b FROM r JOIN w ON w.a = r.x) SELECT s FROM r WHERE s = x"
    )
}

/// The condition for `t` being `Ready`.
#[must_use]
pub fn ready() -> String {
    let (cycle, w) = (on_a_cycle(), waits());
    let (pj, plive) = (joins("p"), alive("p"));
    let (qj, qlive) = (joins("q"), alive("q"));
    format!(
        "(t.progress IN ('needs-action', 'in-process') \
         AND t.uid NOT IN ({cycle}) \
         AND NOT EXISTS (SELECT 1 FROM tasks_dependency d WHERE d.task = t.id AND NOT EXISTS \
           (SELECT 1 FROM tasks_task q {qj} WHERE q.uid = d.other_uid AND {qlive})) \
         AND NOT EXISTS (SELECT 1 FROM ({w}) w JOIN tasks_task p ON p.uid = w.pred {pj} \
           WHERE w.waiter = t.uid AND {plive} AND NOT (p.progress = 'completed' \
             OR w.kind IN ('start-to-start', 'start-to-finish') AND p.progress = 'in-process')))"
    )
}
