//! The rights per list of `LocalTasks` (who sees a list, who writes, who manages, an AI at most `Write`) written as one
//! check that returns the rules it saw broken; the good provider breaks none, each deliberate fault (`Flaw`) breaks its
//! own (Opus review K2a, B1: a rule that stays green against its mutant proves nothing).
//!
//! Pattern: Mutation testing by switch (`LocalTasks::set_flaw`), the same idea as the mutants of the fake.

#[allow(dead_code)]
mod common;

use common::{LocalFx, draft, known};
use fs_tasks_local::Flaw;
use fs_tasks_testdata::tasks_v1::{
    Error, ListAccess, SeriesScope, TaskError, TaskListPatch, TaskPatch, Tasks, Trash,
};

fn forbidden<T>(r: &Result<T, TaskError>) -> bool {
    matches!(r, Err(TaskError::Common(Error::Forbidden { .. })))
}

/// The rules of the rights, run against `fx`; the broken ones by name.
async fn broken(fx: &LocalFx) -> Vec<&'static str> {
    let mut out = Vec::new();
    let (colleague, agent) = (known(2), known(8));
    let list = fx.tasks.create_list("Mine", None, None).await.unwrap();
    let t = fx
        .tasks
        .create(draft(list.id, "T", TaskPatch::default()))
        .await
        .unwrap();
    fx.tasks.act_as(Some(colleague.clone()));
    let unseen = fx.tasks.lists(Trash::Include).await.unwrap().is_empty();
    fx.tasks.act_as(None);
    if !unseen {
        out.push("a list nobody granted is invisible to others");
    }
    fx.tasks
        .grant(&colleague, list.id, ListAccess::Write)
        .await
        .unwrap();
    fx.tasks.act_as(Some(colleague.clone()));
    let seen = fx.tasks.lists(Trash::Exclude).await.unwrap();
    let wrote = fx
        .tasks
        .create(draft(list.id, "by the colleague", TaskPatch::default()))
        .await;
    fx.tasks.act_as(None);
    if seen.len() != 1 || seen[0].access != ListAccess::Write || wrote.is_err() {
        out.push("a grant of Write shows the list and lets the grantee write");
    }
    fx.tasks.make_read_only(&fx.account, list.id).await.unwrap();
    let own = fx
        .tasks
        .create(draft(list.id, "owner read only", TaskPatch::default()))
        .await;
    if !forbidden(&own) {
        out.push("a grant of Read replaces the Manage of the owner (writing is Forbidden)");
    }
    fx.tasks
        .grant(&fx.account, list.id, ListAccess::Manage)
        .await
        .unwrap();
    fx.tasks
        .grant(&agent, list.id, ListAccess::Manage)
        .await
        .unwrap();
    fx.tasks.act_as(Some(agent));
    let edit = fx
        .tasks
        .update_list(list.id, list.revision, TaskListPatch::default())
        .await;
    let del = fx.tasks.delete_list(list.id, list.revision).await;
    let purge = fx.tasks.purge(t.id).await;
    let work = fx
        .tasks
        .update(t.id, t.revision, TaskPatch::default(), SeriesScope::Whole)
        .await;
    fx.tasks.act_as(None);
    if !(forbidden(&edit) && forbidden(&del) && forbidden(&purge)) {
        out.push("an AI is held at Write: even a grant of Manage gives it no update_list, delete_list or purge");
    }
    if work.is_err() {
        out.push("an AI with Manage granted still writes tasks");
    }
    out
}

#[tokio::test]
async fn the_rights_per_list_hold() {
    let fx = LocalFx::memory().await;
    assert_eq!(broken(&fx).await, Vec::<&str>::new());
}

#[tokio::test]
async fn an_ai_without_the_write_cap_is_caught() {
    let fx = LocalFx::memory().await;
    fx.tasks.set_flaw(Some(Flaw::NoAiCap));
    let rules = broken(&fx).await;
    assert!(
        rules
            .iter()
            .any(|r| r.starts_with("an AI is held at Write")),
        "{rules:?}"
    );
}

#[tokio::test]
async fn grants_that_are_ignored_are_caught() {
    let fx = LocalFx::memory().await;
    fx.tasks.set_flaw(Some(Flaw::GrantIgnored));
    let rules = broken(&fx).await;
    assert!(
        rules.iter().any(|r| r.starts_with("a grant of Write")),
        "{rules:?}"
    );
    assert!(
        rules.iter().any(|r| r.starts_with("a grant of Read")),
        "{rules:?}"
    );
}
