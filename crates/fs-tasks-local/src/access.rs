//! What the asking person may do with a list: see it (owner, or a grant), write, manage; an AI at most `Write`.
//!
//! Pattern: Policy (one function says the level, the others turn it into `NotFound`/`Forbidden`). `NotFound` first: a
//! list the asker may not see does not exist for them (nothing leaks through the error). A grant row replaces the
//! owner's level (the owner who is made `Read` only reads); without one the owner has `Manage`.

use fs_objects::v5;
use fs_objects::v5::codec::{damaged, get_opt_text, get_text, id_value, text};
use fs_tasks_testdata::tasks_v1::{BasicValue, ListAccess, ObjectId, PersonRef, TaskResult};

use crate::error::{adapt, forbidden, gone};
use crate::who::Who;

/// The words of a grant level.
pub const LEVELS: &[&str] = &["read", "write", "manage"];

/// The level of a stored word. A word nobody knows is a damaged row (`Unavailable`), never a guess: guessing `Manage`
/// would open a list that was meant to be closed.
pub fn level_of(word: &str) -> TaskResult<ListAccess> {
    match word {
        "read" => Ok(ListAccess::Read),
        "write" => Ok(ListAccess::Write),
        "manage" => Ok(ListAccess::Manage),
        _ => Err(adapt(damaged("grant level"))),
    }
}

/// The levels of the asker: on `list` only, or on every list (`None`); a list they may not see is left out. One
/// `SELECT` over the lists and the asker's grant rows.
async fn levels(
    c: &v5::Conn<'_>,
    who: &Who,
    list: Option<ObjectId>,
) -> TaskResult<Vec<(ObjectId, ListAccess)>> {
    let me = who.me.id.canonical();
    let mut sql = "SELECT l.id, l.owner_id, g.level FROM tasks_list l \
        LEFT JOIN tasks_grant g ON g.list = l.id AND g.who_id = ?"
        .to_owned();
    let mut params = vec![text(&me)];
    if let Some(l) = list {
        sql.push_str(" WHERE l.id = ?");
        params.push(id_value(l));
    }
    sql.push_str(" ORDER BY l.id");
    let rows = c.query(&sql, params).await.map_err(adapt)?;
    let mut out = Vec::new();
    for r in &rows {
        let id = v5::codec::get_id(r, 0).map_err(adapt)?;
        let owner = get_text(r, 1).map_err(adapt)? == me;
        let grant = get_opt_text(r, 2)
            .map_err(adapt)?
            .filter(|_| !who.grants_ignored());
        let level = match grant.as_deref() {
            Some(word) => level_of(word)?,
            None if owner => ListAccess::Manage,
            None => continue,
        };
        out.push((
            id,
            if who.asks_as_agent() && !who.ai_uncapped() {
                level.min(ListAccess::Write)
            } else {
                level
            },
        ));
    }
    Ok(out)
}

/// The level of the asker on `list`; `None` = they may not see it (or there is no such list).
pub async fn level(c: &v5::Conn<'_>, who: &Who, list: ObjectId) -> TaskResult<Option<ListAccess>> {
    Ok(levels(c, who, Some(list)).await?.first().map(|l| l.1))
}

/// The asker needs at least `want` on `list`: `NotFound` when they cannot see it, `Forbidden { cap }` when they can but
/// may not.
pub async fn need(
    c: &v5::Conn<'_>,
    who: &Who,
    list: ObjectId,
    want: ListAccess,
    cap: &str,
) -> TaskResult<()> {
    match level(c, who, list).await? {
        None => Err(gone(list)),
        Some(l) if l < want => Err(forbidden(cap)),
        Some(_) => Ok(()),
    }
}

/// The lists the asker may see (living or in the trash), with their level, by id.
pub async fn visible(c: &v5::Conn<'_>, who: &Who) -> TaskResult<Vec<(ObjectId, ListAccess)>> {
    levels(c, who, None).await
}

/// A person gets `level` on a list (the fixture and later the sharing of lists use this): a grant row, replacing any older one.
pub async fn set_grant(
    c: &v5::Conn<'_>,
    who: &PersonRef,
    list: ObjectId,
    level: ListAccess,
) -> TaskResult<()> {
    c.exec(
        "DELETE FROM tasks_grant WHERE list = ? AND who_id = ?",
        vec![id_value(list), text(&who.id.canonical())],
    )
    .await
    .map_err(adapt)?;
    let [id, art, abzug] = v5::codec::person_values(who);
    let word = match level {
        ListAccess::Read => "read",
        ListAccess::Write => "write",
        _ => "manage",
    };
    c.insert(
        &crate::tables::GRANT,
        vec![id_value(list), id, art, abzug, text(word)],
    )
    .await
    .map_err(adapt)?;
    Ok(())
}
