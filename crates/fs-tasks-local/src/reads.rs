//! Reading one task: `get` and `get_by_uid` (a living task of a list the asker may see; anything else is `None`).
//!
//! Pattern: Query functions (read-only, no unit of work; the deadline run is the caller's).

use fs_objects::v5;
use fs_objects::v5::codec::text;
use fs_tasks_testdata::tasks_v1::{ObjectId, Task, TaskResult};

use crate::access::level;
use crate::stored::{Stored, load, load_one};
use crate::who::Who;

/// `Some` when `s` is alive and its list is visible.
async fn shown(c: &v5::Conn<'_>, who: &Who, s: Stored) -> TaskResult<Option<Task>> {
    if !s.alive() || level(c, who, s.task.list).await?.is_none() {
        return Ok(None);
    }
    Ok(Some(s.task))
}

/// `get`.
pub async fn get(c: &v5::Conn<'_>, who: &Who, id: ObjectId) -> TaskResult<Option<Task>> {
    match load_one(c, id).await? {
        Some(s) => shown(c, who, s).await,
        None => Ok(None),
    }
}

/// `get_by_uid`.
pub async fn get_by_uid(c: &v5::Conn<'_>, who: &Who, uid: &str) -> TaskResult<Option<Task>> {
    for s in load(c, "t.uid = ?", vec![text(uid)]).await? {
        if let Some(t) = shown(c, who, s).await? {
            return Ok(Some(t));
        }
    }
    Ok(None)
}
