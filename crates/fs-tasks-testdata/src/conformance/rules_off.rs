//! Rules for a staged provider: every capability it switched off is CALLED once and must answer `Unsupported`
//! (Opus review K1-1, A1). A group that is skipped because its capability is off proves nothing about the refusal; a
//! provider that answers a switched-off method with a quiet `Ok` would stay green without this rule.
//!
//! Pattern: Template method (one call per method of a switched-off capability, collected in the report).

use super::cx::{Cx, is_unsupported};
use crate::tasks_v1::{
    Answer, CAPABILITIES, ChecklistChange, Children, Dependency, DependencyKind, ObjectId,
    Progress, Revision, SeriesScope, TaskDraft, TaskListPatch, TaskPatch, TaskResult, Tasks, Utc,
};

fn no<T>(r: &TaskResult<T>) -> bool {
    r.as_ref().err().is_some_and(is_unsupported)
}

/// One call per method of a switched-off capability (with the task and list at hand); `cx.ensure` for each.
async fn calls(cx: &mut Cx<'_>, off: &[&str], list: ObjectId, id: ObjectId, rev: Revision) {
    let d: &dyn Tasks = cx.dir;
    let me = d.asker().await.unwrap_or_else(|_| crate::ids::person(1));
    let (w, until) = (SeriesScope::Whole, Utc::from_millis(1));
    let dep = || Dependency {
        other_uid: "urn:uuid:none".into(),
        kind: DependencyKind::DependsOn,
        gap: None,
    };
    let draft = TaskDraft {
        list,
        uid: None,
        title: "off".to_owned(),
        progress: Progress::NeedsAction,
        assignees: Vec::new(),
        checklist: Vec::new(),
        rest: TaskPatch::default(),
    };
    let item = ChecklistChange::Remove { key: "k".into() };
    let q = cx.query(&[list]);
    let mut seen: Vec<(&str, &str, bool)> = Vec::new();
    macro_rules! call {
        ($cap:expr, $method:expr, $e:expr) => {
            if off.contains(&$cap) {
                seen.push(($cap, $method, no(&$e.await)));
            }
        };
    }
    call!(
        "list.create",
        "create_list",
        d.create_list("off", None, None)
    );
    call!(
        "list.edit",
        "update_list",
        d.update_list(list, rev, TaskListPatch::default())
    );
    call!("task.labels", "labels", d.labels(&[], None, None, 1));
    call!("dependency.evaluate", "readiness", d.readiness(&[id]));
    call!("task.create", "create", d.create(draft));
    call!(
        "task.edit",
        "update",
        d.update(id, rev, TaskPatch::default(), w)
    );
    call!("task.assign", "assign", d.assign(id, rev, &me, w));
    call!("task.assign", "unassign", d.unassign(id, rev, &me, w));
    call!(
        "task.respond",
        "respond",
        d.respond(id, rev, Answer::Accept, None, w)
    );
    call!(
        "task.progress",
        "progress",
        d.progress(id, rev, Progress::Completed, Children::Leave, w)
    );
    call!("task.place", "place", d.place(id, rev, None));
    call!("task.checklist", "checklist", d.checklist(id, rev, item));
    call!("relation.create", "relate", d.relate(id, rev, dep()));
    call!(
        "relation.delete",
        "unrelate",
        d.unrelate(id, rev, "urn:uuid:none", DependencyKind::DependsOn)
    );
    call!("task.claim", "claim", d.claim(id, rev, until));
    call!("task.claim", "release", d.release(id, rev));
    call!("task.claim", "claim_next", d.claim_next(&q, until));
    call!("task.history", "history", d.history(id, None, 1));
    call!("task.changes", "changes", d.changes(None, 1));
    call!("task.restore", "restore", d.restore(id, w));
    call!("task.delete", "delete", d.delete(id, rev, w));
    call!("trash.purge", "purge", d.purge(id));
    call!("list.delete", "delete_list", d.delete_list(list, rev));
    call!("list.delete", "restore_list", d.restore_list(list));
    for (cap, method, unsupported) in seen {
        cx.ensure(
            unsupported,
            &format!("{method}: the switched-off capability {cap} answers Unsupported"),
        );
    }
}

/// A staged provider only: each switched-off capability answers `Unsupported` to every method it covers.
pub(super) async fn rules(cx: &mut Cx<'_>) {
    if !cx.partial {
        return;
    }
    let off: Vec<&str> = CAPABILITIES
        .iter()
        .copied()
        .filter(|c| !cx.caps(c))
        .collect();
    if off.is_empty() {
        return;
    }
    // A real task, so that a quiet `Ok` has something to answer with (the calls that would change it come last).
    let (list, id, rev) = match cx.list("off").await {
        Some(l) => match cx.task(l.id, "off").await {
            Some(t) => (l.id, t.id, t.revision),
            None => (l.id, l.id, l.revision),
        },
        None => (
            crate::ids::object_id(1),
            crate::ids::object_id(1),
            Revision(1),
        ),
    };
    calls(cx, &off, list, id, rev).await;
}
