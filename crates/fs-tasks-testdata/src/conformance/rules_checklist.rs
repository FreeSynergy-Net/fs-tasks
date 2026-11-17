//! Rules for the checklist of a task: items in order, tick, move, limit, log, search, and that they are no tasks.
//!
//! Pattern: Template method (rules as plain checks collected in the report).

use super::cx::{Cx, draft, is_changed, is_invalid};
use crate::tasks_v1::{CHECKLIST_MAX, ChecklistChange, Task, TaskDraft, TaskPatch};

fn titles(t: &Task) -> Vec<&str> {
    t.checklist.iter().map(|c| c.title.as_str()).collect()
}

pub(super) async fn rules(cx: &mut Cx<'_>) {
    if !cx.caps("task.checklist") {
        cx.skip("checklist: the provider has no task.checklist");
        return;
    }
    let Some(list) = cx.list("checklist").await else {
        return;
    };
    let dir = cx.dir;
    let made = dir
        .create(TaskDraft {
            checklist: vec!["eins".into(), "zwei".into(), "drei".into()],
            ..draft(list.id, "mit Punkten", TaskPatch::default())
        })
        .await;
    let Ok(t) = made else {
        cx.ensure(false, "create: a draft with checklist titles");
        return;
    };
    let keys: Vec<&str> = t.checklist.iter().map(|c| c.key.as_str()).collect();
    cx.ensure(
        titles(&t) == ["eins", "zwei", "drei"] && t.checklist.iter().all(|c| c.done.is_none()),
        "create: the checklist titles in this order, all open",
    );
    cx.ensure(
        keys.iter().enumerate().all(|(i, k)| !keys[..i].contains(k)),
        "create: every item has its own key",
    );
    let sum = cx.summary(list.id, t.id).await;
    cx.ensure(
        sum.is_some_and(|s| s.checklist_total == 3 && s.checklist_open == 3),
        "list: the card says 3 of 3 open",
    );
    let empty = dir
        .create(TaskDraft {
            checklist: vec![" ".into()],
            ..draft(list.id, "leer", TaskPatch::default())
        })
        .await;
    cx.fails(&empty, "create: an empty item title is Invalid", is_invalid);
    let many = dir
        .create(TaskDraft {
            checklist: vec!["x".into(); CHECKLIST_MAX + 1],
            ..draft(list.id, "zu viele", TaskPatch::default())
        })
        .await;
    cx.fails(
        &many,
        "create: more than CHECKLIST_MAX items is Invalid",
        is_invalid,
    );
    edits(cx, t).await;
    super::rules_checklist_more::limit(cx, list.id).await;
}

async fn edits(cx: &mut Cx<'_>, t: Task) {
    let dir = cx.dir;
    let add = |title: &str, after: Option<&str>| ChecklistChange::Add {
        title: title.to_owned(),
        after: after.map(str::to_owned),
    };
    let Ok(top) = dir.checklist(t.id, t.revision, add("null", None)).await else {
        cx.ensure(false, "checklist: Add at the top answers");
        return;
    };
    cx.ensure(
        titles(&top) == ["null", "eins", "zwei", "drei"] && top.revision > t.revision,
        "checklist: Add with after None goes to the top and raises the revision of the task",
    );
    let k = |t: &Task, i: usize| t.checklist[i].key.clone();
    let Ok(mid) = dir
        .checklist(top.id, top.revision, add("eins-b", Some(&k(&top, 1))))
        .await
    else {
        cx.ensure(false, "checklist: Add after a key answers");
        return;
    };
    cx.ensure(
        titles(&mid) == ["null", "eins", "eins-b", "zwei", "drei"],
        "checklist: Add after a key goes right behind it",
    );
    let stale = dir.checklist(mid.id, t.revision, add("x", None)).await;
    cx.fails(&stale, "checklist: a stale revision is Changed", is_changed);
    let unknown = dir
        .checklist(
            mid.id,
            mid.revision,
            ChecklistChange::Check {
                key: "nope".into(),
                done: true,
            },
        )
        .await;
    cx.fails(&unknown, "checklist: an unknown key is Invalid", is_invalid);
    let blank = dir.checklist(mid.id, mid.revision, add("  ", None)).await;
    cx.fails(&blank, "checklist: an empty title is Invalid", is_invalid);
    let key = k(&mid, 2);
    let Ok(named) = dir
        .checklist(
            mid.id,
            mid.revision,
            ChecklistChange::Rename {
                key: key.clone(),
                title: "eins-c".into(),
            },
        )
        .await
    else {
        cx.ensure(false, "checklist: Rename answers");
        return;
    };
    cx.ensure(
        named.checklist[2].title == "eins-c" && named.checklist[2].key == key,
        "checklist: Rename keeps the key and the place",
    );
    let tick = ChecklistChange::Check {
        key: key.clone(),
        done: true,
    };
    let Ok(ticked) = dir.checklist(named.id, named.revision, tick.clone()).await else {
        cx.ensure(false, "checklist: Check answers");
        return;
    };
    let asker = dir.asker().await.ok();
    cx.ensure(
        ticked.checklist[2]
            .done
            .as_ref()
            .is_some_and(|c| Some(&c.by) == asker.as_ref()),
        "checklist: Check records who ticked",
    );
    let again = dir.checklist(ticked.id, ticked.revision, tick).await;
    cx.ensure(
        again.as_ref().is_ok_and(|a| a.revision == ticked.revision),
        "checklist: ticking a ticked item changes nothing, no new revision",
    );
    let summary = cx.summary(t.list, t.id).await;
    cx.ensure(
        summary.is_some_and(|s| s.checklist_total == 5 && s.checklist_open == 4),
        "list: the card says 4 of 5 open",
    );
    let Ok(moved) = dir
        .checklist(
            ticked.id,
            ticked.revision,
            ChecklistChange::Place {
                key: k(&ticked, 0),
                after: Some(k(&ticked, 4)),
            },
        )
        .await
    else {
        cx.ensure(false, "checklist: Place answers");
        return;
    };
    cx.ensure(
        titles(&moved) == ["eins", "eins-c", "zwei", "drei", "null"],
        "checklist: Place moves an item behind another",
    );
    let Ok(gone) = dir
        .checklist(
            moved.id,
            moved.revision,
            ChecklistChange::Remove { key: k(&moved, 4) },
        )
        .await
    else {
        cx.ensure(false, "checklist: Remove answers");
        return;
    };
    cx.ensure(
        titles(&gone) == ["eins", "eins-c", "zwei", "drei"],
        "checklist: Remove takes the item out",
    );
    super::rules_checklist_more::log_and_search(cx, &gone).await;
}
