//! The parts of a task that live in their own tables and are loaded for many tasks at once: dependencies, alerts, links.
//!
//! Pattern: Repository helper (one `SELECT` per table for a whole set of tasks, then handed to each task).

use fs_objects::v5;
use fs_tasks_testdata::tasks_v1::{ObjectId, TaskResult};

use crate::stored::Stored;
use crate::{alerts_rows, deps_rows, links_rows};

/// Fill `dependencies`, `alerts` and `links` of the loaded tasks.
pub async fn attach(c: &v5::Conn<'_>, rows: &mut [Stored]) -> TaskResult<()> {
    let ids: Vec<ObjectId> = rows.iter().map(|s| s.task.id).collect();
    let mut deps = deps_rows::of(c, &ids).await?;
    let mut alerts = alerts_rows::of(c, &ids).await?;
    let mut links = links_rows::of(c, &ids).await?;
    for s in rows {
        let id = s.task.id;
        s.task.dependencies = deps.remove(&id).unwrap_or_default();
        s.task.alerts = alerts.remove(&id).unwrap_or_default();
        s.task.links = links.remove(&id).unwrap_or_default();
    }
    Ok(())
}
