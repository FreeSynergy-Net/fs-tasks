//! The runner for a provider WITHOUT the write capabilities: every method with a standard answer says `Unsupported`.
//!
//! Pattern: Null Object check (one call per method, the answer must be `Unsupported`, and `capabilities` says the same).

use super::cx::{Cx, Options, is_unsupported};
use crate::tasks_v1::{
    Answer, Children, DependencyKind, Progress, SeriesScope, TaskDraft, TaskListPatch, TaskPatch,
    Tasks, Trash,
};

/// Rules for a provider WITHOUT the write capabilities: every method with a standard answer says `Unsupported`
/// (Null Object), and `capabilities` says the same.
pub async fn run_unsupported(dir: &dyn Tasks) -> Vec<String> {
    let Some(mut cx) = Cx::new(dir, Options::default()) else {
        return vec!["setup: the zones UTC and Europe/Berlin".to_owned()];
    };
    let id = crate::ids::object_id(1);
    let rev = crate::tasks_v1::Revision(1);
    let me = crate::ids::person(1);
    let draft = TaskDraft {
        list: id,
        uid: None,
        title: "x".to_owned(),
        progress: Progress::NeedsAction,
        assignees: Vec::new(),
        checklist: Vec::new(),
        rest: TaskPatch::default(),
    };
    let w = SeriesScope::Whole;
    let off: [(&str, bool); 8] = [
        (
            "list.create",
            is_unsupported_of(&dir.create_list("x", None, None).await),
        ),
        (
            "list.edit",
            is_unsupported_of(&dir.update_list(id, rev, TaskListPatch::default()).await),
        ),
        (
            "list.delete",
            is_unsupported_of(&dir.delete_list(id, rev).await),
        ),
        ("task.create", is_unsupported_of(&dir.create(draft).await)),
        (
            "task.edit",
            is_unsupported_of(&dir.update(id, rev, TaskPatch::default(), w).await),
        ),
        (
            "task.assign",
            is_unsupported_of(&dir.assign(id, rev, &me, w).await),
        ),
        (
            "task.respond",
            is_unsupported_of(&dir.respond(id, rev, Answer::Accept, None, w).await),
        ),
        (
            "task.progress",
            is_unsupported_of(
                &dir.progress(id, rev, Progress::Completed, Children::Leave, w)
                    .await,
            ),
        ),
    ];
    for (cap, unsupported) in off {
        cx.ensure(
            unsupported,
            &format!("{cap}: the default answer is Unsupported"),
        );
        let off = !cx.caps(cap);
        cx.ensure(off, &format!("capabilities: {cap} is off"));
    }
    let more = [
        (
            "task.place",
            is_unsupported_of(&dir.place(id, rev, None).await),
        ),
        (
            "relation.create",
            is_unsupported_of(
                &dir.relate(
                    id,
                    rev,
                    crate::tasks_v1::Dependency {
                        other_uid: "u".into(),
                        kind: DependencyKind::DependsOn,
                        gap: None,
                    },
                )
                .await,
            ),
        ),
        (
            "task.claim",
            is_unsupported_of(
                &dir.claim(id, rev, crate::tasks_v1::Utc::from_millis(1))
                    .await,
            ),
        ),
        (
            "task.delete",
            is_unsupported_of(&dir.delete(id, rev, w).await),
        ),
        ("task.restore", is_unsupported_of(&dir.restore(id, w).await)),
        (
            "task.history",
            is_unsupported_of(&dir.history(id, None, 1).await),
        ),
        (
            "task.labels",
            is_unsupported_of(&dir.labels(&[], None, None, 1).await),
        ),
        (
            "dependency.evaluate",
            is_unsupported_of(&dir.readiness(&[id]).await),
        ),
        (
            "task.checklist",
            is_unsupported_of(
                &dir.checklist(
                    id,
                    rev,
                    crate::tasks_v1::ChecklistChange::Remove { key: "k".into() },
                )
                .await,
            ),
        ),
        (
            "task.claim",
            is_unsupported_of(
                &dir.claim_next(&cx.query(&[]), crate::tasks_v1::Utc::from_millis(1))
                    .await,
            ),
        ),
        ("trash.purge", is_unsupported_of(&dir.purge(id).await)),
    ];
    for (cap, unsupported) in more {
        cx.ensure(
            unsupported,
            &format!("{cap}: the default answer is Unsupported"),
        );
    }
    let q = cx.query(&[]);
    cx.ensure(
        dir.lists(Trash::Exclude).await.is_ok() && dir.list(&q, None, 1).await.is_ok(),
        "required methods answer",
    );
    cx.ensure(dir.asker().await.is_ok(), "asker: answers without an error");
    cx.report.failures
}

fn is_unsupported_of<T>(r: &crate::tasks_v1::TaskResult<T>) -> bool {
    r.as_ref().err().is_some_and(is_unsupported)
}
