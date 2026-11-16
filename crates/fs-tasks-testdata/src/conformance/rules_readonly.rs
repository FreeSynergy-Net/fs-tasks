//! Rules for a person who may only read a list (`ListAccess::Read`): seeing works, every write is `Forbidden`.
//!
//! Pattern: Template method (rules as plain checks collected in the report). The fixture makes the list read only
//! after the tasks exist, so the same person has written there before.

use super::ConformanceFixture;
use super::cx::{Cx, is_forbidden};
use crate::tasks_v1::{
    ChecklistChange, Children, Dependency, DependencyKind, ListAccess, Progress, SeriesScope,
    TaskDraft, TaskPatch, Trash,
};

pub(super) async fn rules(fx: &dyn ConformanceFixture, cx: &mut Cx<'_>) {
    let Ok(me) = cx.dir.asker().await else { return };
    let Some(list) = cx.list("read only").await else {
        return;
    };
    let Some(t) = cx.task(list.id, "to read").await else {
        return;
    };
    if !fx.read_only(&me, list.id).await {
        cx.skip("read only: writes are Forbidden (the fixture cannot make a list read only)");
        return;
    }
    let dir = cx.dir;
    let shown = dir.lists(Trash::Exclude).await.unwrap_or_default();
    cx.ensure(
        shown
            .iter()
            .any(|l| l.id == list.id && l.access == ListAccess::Read),
        "read only: the list is shown with the access Read",
    );
    cx.ensure(
        matches!(dir.get(t.id).await, Ok(Some(_))) && cx.summary(list.id, t.id).await.is_some(),
        "read only: reading (get, list) works",
    );
    let w = SeriesScope::Whole;
    let draft = TaskDraft {
        list: list.id,
        uid: None,
        title: "new".to_owned(),
        progress: Progress::NeedsAction,
        assignees: Vec::new(),
        checklist: Vec::new(),
        rest: TaskPatch::default(),
    };
    let me2 = crate::ids::person(4242);
    let far = super::rules_claim::far(&t);
    let forbidden = [
        ("task.create", "create", dir.create(draft).await.map(|_| ())),
        (
            "task.edit",
            "update",
            dir.update(t.id, t.revision, TaskPatch::default(), w)
                .await
                .map(|_| ()),
        ),
        (
            "task.delete",
            "delete",
            dir.delete(t.id, t.revision, w).await.map(|_| ()),
        ),
        (
            "task.progress",
            "progress",
            dir.progress(t.id, t.revision, Progress::Completed, Children::Leave, w)
                .await
                .map(|_| ()),
        ),
        (
            "task.assign",
            "assign",
            dir.assign(t.id, t.revision, &me2, w).await.map(|_| ()),
        ),
        (
            "task.assign",
            "unassign",
            dir.unassign(t.id, t.revision, &me2, w).await.map(|_| ()),
        ),
        (
            "task.claim",
            "claim",
            dir.claim(t.id, t.revision, far).await.map(|_| ()),
        ),
        (
            "task.claim",
            "release",
            dir.release(t.id, t.revision).await.map(|_| ()),
        ),
        (
            "task.place",
            "place",
            dir.place(t.id, t.revision, None).await.map(|_| ()),
        ),
        (
            "relation.create",
            "relate",
            dir.relate(
                t.id,
                t.revision,
                Dependency {
                    other_uid: "zzconf-other".to_owned(),
                    kind: DependencyKind::DependsOn,
                    gap: None,
                },
            )
            .await
            .map(|_| ()),
        ),
        (
            "task.checklist",
            "checklist",
            dir.checklist(
                t.id,
                t.revision,
                ChecklistChange::Add {
                    title: "x".to_owned(),
                    after: None,
                },
            )
            .await
            .map(|_| ()),
        ),
    ];
    for (cap, what, answer) in forbidden {
        if cx.partial && !cx.caps(cap) {
            continue; // a switched-off capability answers Unsupported: `rules_off` calls it
        }
        cx.fails(
            &answer,
            &format!("read only: {what} is Forbidden"),
            is_forbidden,
        );
    }
}
