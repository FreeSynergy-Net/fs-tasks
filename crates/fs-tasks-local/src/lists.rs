//! Lists: read, create, change (name, colour, owner), delete into the trash (with all their tasks) and restore.
//!
//! Pattern: Transaction scripts (each function runs in the caller's unit of work; compare-and-set on the envelope's
//! `rev`). Who may own a list (PLAN EIGENTUM): the asking person themselves; a group or organisation needs
//! `GroupMembers` (run 2b) and is `Forbidden` until then; another person is `Invalid`; an AI owns nothing and asking
//! as an AI is `Forbidden` (the provider knows its AIs from `mark_agent`).

use fs_objects::v5;
use fs_objects::v5::codec::{id_value, opt_text, person_values, text};
use fs_objects::v5::envelope::{bump, insert, trash, untrash};
use fs_objects::v5::new_id;
use fs_tasks_testdata::tasks_v1::{
    Deleted, ListAccess, ListSource, ObjectId, PersonArt, PersonRef, Revision, SeriesScope,
    TaskList, TaskListPatch, TaskResult, Trash,
};
use turso::Value;

use crate::access::{need, visible};
use crate::cur::Cur;
use crate::error::{adapt, bad, changed, forbidden, gone};
use crate::stored::load;
use crate::tables::LIST;
use crate::trash_state::{Gone, gone as gone_of};
use crate::who::Who;

const SELECT: &str = "SELECT l.id, l.name, l.color, l.owner_id, l.owner_art, l.owner_abzug, o.rev, o.deleted, \
    o.purge_after FROM tasks_list l JOIN objects o ON o.id = l.id";

/// The lists that match `cond` (SQL over `l` and `o`), by id.
pub async fn load_lists(
    c: &v5::Conn<'_>,
    cond: &str,
    params: Vec<Value>,
) -> TaskResult<Vec<TaskList>> {
    let rows = c
        .query(&format!("{SELECT} WHERE {cond} ORDER BY l.id"), params)
        .await
        .map_err(adapt)?;
    let mut out = Vec::new();
    for row in &rows {
        let mut cur = Cur::new(row);
        let (id, name, color, owner) = (cur.id()?, cur.text()?, cur.opt_text()?, cur.person()?);
        let revision = Revision(u64::try_from(cur.int()?).unwrap_or(0));
        let trash = gone_of(cur.opt_int()?, cur.opt_int()?).map(Gone::restorable);
        out.push(TaskList {
            id,
            name,
            color,
            owner,
            access: ListAccess::Manage,
            source: ListSource::Own,
            revision,
            trash,
        });
    }
    Ok(out)
}

/// The lists the asking person sees (their own and those granted to them), with their access, by trash view.
pub async fn lists(c: &v5::Conn<'_>, who: &Who, view: Trash) -> TaskResult<Vec<TaskList>> {
    let cond = match view {
        Trash::Exclude => "o.deleted IS NULL",
        Trash::Include => "1 = 1",
        Trash::Only => "o.deleted IS NOT NULL",
    };
    let seen = visible(c, who).await?;
    let mut out = load_lists(c, cond, Vec::new()).await?;
    out.retain_mut(|l| {
        seen.iter().find(|s| s.0 == l.id).is_some_and(|s| {
            l.access = s.1;
            true
        })
    });
    Ok(out)
}

/// A list in any trash state, `NotFound` when there is none.
pub async fn any(c: &v5::Conn<'_>, id: ObjectId) -> TaskResult<TaskList> {
    load_lists(c, "l.id = ?", vec![id_value(id)])
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| gone(id))
}

/// A living list, `NotFound` when there is none or it is in the trash.
pub async fn live(c: &v5::Conn<'_>, id: ObjectId) -> TaskResult<TaskList> {
    let list = any(c, id).await?;
    if list.trash.is_some() {
        return Err(gone(id));
    }
    Ok(list)
}

/// Who may own a list: the asker (not an AI); a group needs `GroupMembers` (run 2b), another person is `Invalid`.
fn check_owner(who: &Who, owner: &PersonRef, capability: &str) -> TaskResult<()> {
    if matches!(owner.art, PersonArt::Group | PersonArt::Organization)
        || who.is_agent(owner)
        || who.asks_as_agent()
    {
        return Err(forbidden(capability));
    }
    if owner.id == who.me.id {
        Ok(())
    } else {
        Err(bad("owner", "other-person"))
    }
}

fn check_name(name: &str) -> TaskResult<()> {
    if name.trim().is_empty() {
        return Err(bad("name", "empty"));
    }
    Ok(())
}

/// `create_list`.
pub async fn create(
    c: &v5::Conn<'_>,
    who: &Who,
    now: i64,
    name: &str,
    color: Option<&str>,
    owner: Option<&PersonRef>,
) -> TaskResult<TaskList> {
    check_name(name)?;
    let owner = owner.unwrap_or(&who.me);
    check_owner(who, owner, "list.create")?;
    let id = new_id(now).map_err(adapt)?;
    insert(c, id, "list", name, &who.me, now)
        .await
        .map_err(adapt)?;
    let [oid, oart, oabzug] = person_values(owner);
    c.insert(
        &LIST,
        vec![id_value(id), text(name), opt_text(color), oid, oart, oabzug],
    )
    .await
    .map_err(adapt)?;
    any(c, id).await
}

/// `update_list`: `NotFound`, then `Changed`, then `Invalid`; nothing changed = no new revision.
pub async fn update(
    c: &v5::Conn<'_>,
    who: &Who,
    now: i64,
    id: ObjectId,
    expected: Revision,
    patch: TaskListPatch,
) -> TaskResult<TaskList> {
    need(c, who, id, ListAccess::Manage, "list.edit").await?;
    let old = live(c, id).await?;
    if old.revision != expected {
        return Err(changed(old.revision));
    }
    if let Some(n) = &patch.name {
        check_name(n)?;
    }
    if let Some(o) = &patch.owner {
        check_owner(who, o, "list.edit")?;
    }
    let name = patch.name.unwrap_or_else(|| old.name.clone());
    let color = patch.color.unwrap_or_else(|| old.color.clone());
    let owner = patch.owner.unwrap_or_else(|| old.owner.clone());
    if name == old.name && color == old.color && owner == old.owner {
        return Ok(old);
    }
    bump(c, id, expected, Some(&name), now)
        .await
        .map_err(adapt)?;
    let [oid, oart, oabzug] = person_values(&owner);
    c.exec(
        "UPDATE tasks_list SET name = ?, color = ?, owner_id = ?, owner_art = ?, owner_abzug = ? WHERE id = ?",
        vec![text(&name), opt_text(color.as_deref()), oid, oart, oabzug, id_value(id)],
    )
    .await
    .map_err(adapt)?;
    any(c, id).await
}

/// `delete_list`: the list goes into the trash with every living task in it (also subtasks), one deadline.
pub async fn delete(
    c: &v5::Conn<'_>,
    who: &Who,
    now: i64,
    id: ObjectId,
    expected: Revision,
) -> TaskResult<Deleted> {
    need(c, who, id, ListAccess::Manage, "list.delete").await?;
    let old = live(c, id).await?;
    if old.revision != expected {
        return Err(changed(old.revision));
    }
    let tasks = load(c, "t.list = ?", vec![id_value(id)]).await?;
    let with_subtasks: Vec<ObjectId> = tasks
        .iter()
        .filter(|s| s.own.is_none() && s.with.is_none())
        .map(|s| s.task.id)
        .collect();
    let (_, until) = trash(c, id, expected, now).await.map_err(adapt)?;
    Ok(Deleted {
        id,
        scope: SeriesScope::Whole,
        with_subtasks,
        restorable: Some(
            Gone {
                deleted: now,
                until,
            }
            .restorable(),
        ),
    })
}

/// `restore_list`: only while the deadline holds; a living list is `NotFound`.
pub async fn restore(c: &v5::Conn<'_>, who: &Who, now: i64, id: ObjectId) -> TaskResult<TaskList> {
    need(c, who, id, ListAccess::Manage, "list.delete").await?;
    let old = any(c, id).await?;
    if old.trash.is_none() {
        return Err(gone(id));
    }
    untrash(c, id, now).await.map_err(adapt)?;
    any(c, id).await
}
