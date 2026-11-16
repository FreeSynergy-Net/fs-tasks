//! More rules for the checklist: the log, the text search, the trash, and the limit.
//!
//! Pattern: Template method (rules as plain checks collected in the report).

use super::cx::{Cx, is_invalid};
use crate::tasks_v1::{CHECKLIST_MAX, ChecklistChange, HistoryKind, ObjectId, Task, Trash};

fn titles(t: &Task) -> Vec<&str> {
    t.checklist.iter().map(|c| c.title.as_str()).collect()
}

/// The log (`row_key` = key), the text search over items, and that items are no rows of `list`.
pub(super) async fn log_and_search(cx: &mut Cx<'_>, t: &Task) {
    let dir = cx.dir;
    if cx.caps("task.history") {
        let log = dir
            .history(t.id, None, 100)
            .await
            .map(|p| p.items)
            .unwrap_or_default();
        let has = |kind: HistoryKind, field: &str| {
            log.iter().any(|e| {
                e.kind == kind
                    && e.changes
                        .iter()
                        .any(|c| c.field == field && c.row_key.is_some())
            })
        };
        cx.ensure(
            has(HistoryKind::Add, "checklist")
                && has(HistoryKind::Remove, "checklist")
                && has(HistoryKind::Update, "checklist.title")
                && has(HistoryKind::Update, "checklist.done")
                && has(HistoryKind::Move, "checklist.rank"),
            "history: Add, Remove, Update (title, done) and Move of items, each with the key as row_key",
        );
    }
    let mut q = cx.query(&[t.list]);
    q.text = Some("EINS-C".to_owned());
    cx.ensure(
        cx.ids(&q).await == [t.id],
        "list: the text search finds a task by an item of its checklist",
    );
    q.text = None;
    cx.ensure(
        cx.all(&q).await.len() == 1,
        "list: items are no rows (the one task of the list, not its items)",
    );
    delete_restore(cx, t).await;
}

/// The items go into the trash and come back with their task.
async fn delete_restore(cx: &mut Cx<'_>, t: &Task) {
    let dir = cx.dir;
    let Some(now) = cx.fresh(t.id).await else {
        return;
    };
    let Ok(d) = dir
        .delete(now.id, now.revision, crate::tasks_v1::SeriesScope::Whole)
        .await
    else {
        return;
    };
    let mut q = cx.query(&[t.list]);
    q.trash = Trash::Only;
    cx.ensure(
        cx.summary_in(&q, d.id)
            .await
            .is_some_and(|s| s.checklist_total == 4),
        "list(Only): a deleted task still shows its items",
    );
    let back = dir.restore(d.id, crate::tasks_v1::SeriesScope::Whole).await;
    cx.ensure(
        back.is_ok_and(|b| titles(&b) == ["eins", "eins-c", "zwei", "drei"]),
        "restore: the items come back with the task",
    );
}

/// Exactly `CHECKLIST_MAX` items fit, one more is `Invalid`.
pub(super) async fn limit(cx: &mut Cx<'_>, list: ObjectId) {
    let dir = cx.dir;
    let Some(t) = cx.task(list, "voll").await else {
        return;
    };
    let mut rev = t.revision;
    for i in 0..CHECKLIST_MAX {
        let r = dir
            .checklist(
                t.id,
                rev,
                ChecklistChange::Add {
                    title: format!("p{i}"),
                    after: None,
                },
            )
            .await;
        let Ok(x) = r else {
            cx.ensure(false, "checklist: CHECKLIST_MAX items fit");
            return;
        };
        rev = x.revision;
    }
    let over = dir
        .checklist(
            t.id,
            rev,
            ChecklistChange::Add {
                title: "zu viel".into(),
                after: None,
            },
        )
        .await;
    cx.fails(
        &over,
        "checklist: item number CHECKLIST_MAX + 1 is Invalid",
        is_invalid,
    );
    let full = cx.fresh(t.id).await;
    cx.ensure(
        full.is_some_and(|f| f.checklist.len() == CHECKLIST_MAX && f.checklist.windows(2).all(|w| w[0].rank < w[1].rank)),
        "checklist: 100 items, ranks ascending (the top is hammered, so the items were spread once)",
    );
}
