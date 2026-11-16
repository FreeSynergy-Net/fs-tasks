//! Rules for lists: create, edit, and the trash of a list with its tasks.
//!
//! Pattern: Template method (rules as plain checks collected in the report).

use super::cx::{Cx, is_changed, is_invalid, is_not_found};
use crate::tasks_v1::{ListAccess, Revision, SeriesScope, TaskListPatch, TaskPatch, Trash};

#[allow(clippy::option_option)] // `Some(None)` removes the colour, as in `TaskListPatch`
fn patch(name: Option<&str>, color: Option<Option<&str>>) -> TaskListPatch {
    TaskListPatch {
        name: name.map(str::to_owned),
        color: color.map(|c| c.map(str::to_owned)),
        owner: None,
    }
}

pub(super) async fn rules(cx: &mut Cx<'_>) {
    let dir = cx.dir;
    let made = dir.create_list("Zzconf lists", Some("#112233"), None).await;
    let Ok(list) = made else {
        cx.ensure(false, "create_list: answers");
        return;
    };
    cx.ensure(
        list.name == "Zzconf lists"
            && list.color.as_deref() == Some("#112233")
            && list.trash.is_none(),
        "create_list: name, colour, alive",
    );
    let asker = dir.asker().await.ok();
    cx.ensure(
        asker.as_ref() == Some(&list.owner) && list.access == ListAccess::Manage,
        "create_list: the owner is the asking person, with Manage",
    );
    let stranger = crate::ids::person(4242);
    let other = dir.create_list("Zzconf other", None, Some(&stranger)).await;
    cx.fails(
        &other,
        "create_list: another person as owner is Invalid",
        is_invalid,
    );
    let mine = dir.create_list("Zzconf mine", None, asker.as_ref()).await;
    cx.ensure(
        mine.as_ref()
            .is_ok_and(|l| Some(&l.owner) == asker.as_ref()),
        "create_list: the asking person may be named as owner",
    );
    let empty = dir.create_list("  ", None, None).await;
    cx.fails(&empty, "create_list: an empty name is Invalid", is_invalid);

    let renamed = dir
        .update_list(
            list.id,
            list.revision,
            patch(Some("Zzconf lists 2"), Some(None)),
        )
        .await;
    let Ok(renamed) = renamed else {
        cx.ensure(false, "update_list: renames and removes the colour");
        return;
    };
    cx.ensure(
        renamed.name == "Zzconf lists 2"
            && renamed.color.is_none()
            && renamed.revision > list.revision,
        "update_list: renamed, colour removed, revision up",
    );
    let stale = dir
        .update_list(list.id, list.revision, patch(Some("x"), None))
        .await;
    cx.fails(
        &stale,
        "update_list: a stale revision is Changed",
        is_changed,
    );
    let blank = dir
        .update_list(list.id, renamed.revision, patch(Some(""), None))
        .await;
    cx.fails(&blank, "update_list: an empty name is Invalid", is_invalid);
    let gone = dir
        .update_list(
            crate::ids::object_id(u64::MAX),
            Revision(1),
            patch(None, None),
        )
        .await;
    cx.fails(
        &gone,
        "update_list: an unknown list is NotFound",
        is_not_found,
    );
    trash_rules(cx, renamed.id, renamed.revision).await;
}

/// Deleting a list takes its tasks (also subtasks) along; one deadline for all; restore brings them back.
async fn trash_rules(cx: &mut Cx<'_>, list: crate::tasks_v1::ObjectId, revision: Revision) {
    let dir = cx.dir;
    let (Some(a), Some(b), Some(single)) = (
        cx.task(list, "A").await,
        cx.task(list, "B").await,
        cx.task(list, "single").await,
    ) else {
        return;
    };
    let kid = cx
        .task_with(
            list,
            "kid",
            crate::tasks_v1::TaskPatch {
                parent: Some(Some(a.id)),
                ..TaskPatch::default()
            },
        )
        .await;
    let Some(kid) = kid else { return };
    let single_gone = dir
        .delete(single.id, single.revision, SeriesScope::Whole)
        .await;
    cx.ensure(single_gone.is_ok(), "setup: delete a task on its own");

    let deleted = dir.delete_list(list, revision).await;
    let Ok(deleted) = deleted else {
        cx.ensure(false, "delete_list: answers");
        return;
    };
    let mut want = vec![a.id, b.id, kid.id];
    want.sort();
    let mut got = deleted.with_subtasks.clone();
    got.sort();
    cx.ensure(
        got == want,
        "delete_list: with_subtasks = all living tasks of the list, subtasks too",
    );
    cx.ensure(deleted.restorable.is_some(), "delete_list: a deadline");
    let alive = dir.lists(Trash::Exclude).await.unwrap_or_default();
    let all = dir.lists(Trash::Include).await.unwrap_or_default();
    let only = dir.lists(Trash::Only).await.unwrap_or_default();
    cx.ensure(
        !alive.iter().any(|l| l.id == list),
        "lists(Exclude): not the deleted list",
    );
    cx.ensure(
        all.iter().any(|l| l.id == list),
        "lists(Include): the deleted list",
    );
    cx.ensure(
        only.iter()
            .any(|l| l.id == list && l.trash == deleted.restorable),
        "lists(Only): the deleted list with its deadline",
    );
    cx.ensure(
        dir.get(a.id).await.ok().flatten().is_none(),
        "get: a task of a deleted list is None",
    );
    let mut q = cx.query(&[list]);
    q.trash = Trash::Only;
    let in_trash = cx.all(&q).await;
    let with_list = in_trash
        .iter()
        .filter(|s| s.trash == deleted.restorable)
        .count();
    cx.ensure(
        in_trash.len() == 4 && with_list == 3,
        "list(Only): the tasks of the list with the list's deadline, the one deleted earlier with its own",
    );
    let one = dir.restore(a.id, SeriesScope::Whole).await;
    cx.fails(
        &one,
        "restore: a task of a deleted list is Invalid",
        is_invalid,
    );
    let new = cx
        .dir
        .create(super::cx::draft(list, "late", TaskPatch::default()))
        .await;
    cx.fails(&new, "create: in a deleted list is NotFound", is_not_found);
    let edit = dir
        .update_list(list, Revision(1), patch(Some("x"), None))
        .await;
    cx.ensure(edit.is_err(), "update_list: a deleted list is not edited");

    let back = dir.restore_list(list).await;
    cx.ensure(
        back.as_ref().is_ok_and(|l| l.trash.is_none()),
        "restore_list: the list is alive again",
    );
    cx.ensure(
        dir.get(a.id).await.ok().flatten().is_some()
            && dir.get(kid.id).await.ok().flatten().is_some(),
        "restore_list: the tasks that lay in it come back",
    );
    cx.ensure(
        dir.get(single.id).await.ok().flatten().is_none(),
        "restore_list: a task deleted on its own stays deleted",
    );
    let again = dir.restore_list(list).await;
    cx.fails(
        &again,
        "restore_list: a living list is NotFound",
        is_not_found,
    );
}
