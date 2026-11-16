//! The context of a conformance run: the provider, the report and the small helpers the rules share.
//!
//! Pattern: Test helper (plain functions; a setup that fails is a failure of the rule "setup", never a panic).

use crate::tasks_v1::{
    Change, Error, ObjectId, Order, Place, Progress, Task, TaskDraft, TaskError, TaskList,
    TaskPatch, TaskQuery, TaskResult, TaskSummary, Tasks, Trash, Zone,
};

/// The failed and the skipped rules of a run; empty `failures` means conformant.
#[derive(Debug, Default)]
pub struct Report {
    /// Rules that failed.
    pub failures: Vec<String>,
    /// Rules a fixture could not set up (it has no such mutator); not a failure, but not proven either.
    pub skipped: Vec<String>,
    /// How many checks ran (a rule that returned early because its setup failed is not counted).
    pub checked: usize,
}

/// Provider, report and zones of one run.
pub(crate) struct Cx<'a> {
    pub dir: &'a dyn Tasks,
    pub report: Report,
    pub berlin: Zone,
    /// Insertions at one edge of a rank sequence (2000; 6000 with [`Options::heavy`]).
    pub edge: usize,
    pub skip_rank: bool,
}

/// Switches of a conformance run.
#[derive(Clone, Copy, Debug, Default)]
pub struct Options {
    /// The night run: 6000 insertions at each rank edge instead of 2000 (a provider that writes to a file takes
    /// minutes for the long run; the short one proves the same bound).
    pub heavy: bool,
    /// Leave out the rank rules (they write thousands of tasks): for the quick runs against mutations.
    pub skip_rank: bool,
}

impl<'a> Cx<'a> {
    pub(crate) fn new(dir: &'a dyn Tasks, options: Options) -> Option<Self> {
        Some(Self {
            dir,
            report: Report::default(),
            berlin: Zone::iana("Europe/Berlin").ok()?,
            edge: if options.heavy { 6000 } else { 2000 },
            skip_rank: options.skip_rank,
        })
    }

    pub(crate) fn ensure(&mut self, ok: bool, rule: &str) {
        self.report.checked += 1;
        if !ok {
            self.report.failures.push(rule.to_owned());
        }
    }

    pub(crate) fn skip(&mut self, rule: &str) {
        self.report.skipped.push(rule.to_owned());
    }

    /// The call must fail, and the error must be the wanted one.
    pub(crate) fn fails<T>(
        &mut self,
        result: &TaskResult<T>,
        rule: &str,
        want: fn(&TaskError) -> bool,
    ) {
        let ok = result.as_ref().err().is_some_and(want);
        self.ensure(ok, rule);
    }

    pub(crate) fn caps(&self, name: &str) -> bool {
        self.dir
            .capabilities()
            .0
            .iter()
            .any(|(c, on)| c == name && *on)
    }

    /// A list named `Zzconf <name>`.
    pub(crate) async fn list(&mut self, name: &str) -> Option<TaskList> {
        let made = self
            .dir
            .create_list(&format!("Zzconf {name}"), None, None)
            .await;
        self.ensure(made.is_ok(), &format!("setup: create_list {name}"));
        made.ok()
    }

    pub(crate) async fn task_with(
        &mut self,
        list: ObjectId,
        title: &str,
        rest: TaskPatch,
    ) -> Option<Task> {
        let made = self.dir.create(draft(list, title, rest)).await;
        self.ensure(made.is_ok(), &format!("setup: create {title}"));
        made.ok()
    }

    pub(crate) async fn task(&mut self, list: ObjectId, title: &str) -> Option<Task> {
        self.task_with(list, title, TaskPatch::default()).await
    }

    /// The task as stored now.
    pub(crate) async fn fresh(&self, id: ObjectId) -> Option<Task> {
        self.dir.get(id).await.ok().flatten()
    }

    pub(crate) fn query(&self, lists: &[ObjectId]) -> TaskQuery {
        TaskQuery {
            lists: lists.to_vec(),
            text: None,
            progress: Vec::new(),
            due: None,
            undated: None,
            assignee: None,
            labels: Vec::new(),
            parent: None,
            ready: None,
            order: Order::Rank,
            view_zone: self.berlin.clone(),
            trash: Trash::Exclude,
        }
    }

    /// All summaries of a query, page by page (a provider may cap the page size).
    pub(crate) async fn all(&self, q: &TaskQuery) -> Vec<TaskSummary> {
        let mut out = Vec::new();
        let mut after = None;
        while let Ok(page) = self.dir.list(q, after.as_ref(), 500).await {
            let more = page.next.is_some() && !page.items.is_empty();
            out.extend(page.items);
            after = page.next;
            if !more {
                break;
            }
        }
        out
    }

    /// The changes since `start`, page by page.
    pub(crate) async fn changes_since(&self, start: Option<&Place>) -> Vec<Change> {
        let mut out = Vec::new();
        let mut at = start.cloned();
        while let Ok(page) = self.dir.changes(at.as_ref(), 500).await {
            let empty = page.items.is_empty();
            out.extend(page.items);
            at = page.next;
            if empty || at.is_none() {
                break;
            }
        }
        out
    }

    pub(crate) async fn ids(&self, q: &TaskQuery) -> Vec<ObjectId> {
        self.all(q).await.into_iter().map(|s| s.id).collect()
    }

    pub(crate) async fn summary(&self, list: ObjectId, id: ObjectId) -> Option<TaskSummary> {
        self.all(&self.query(&[list]))
            .await
            .into_iter()
            .find(|s| s.id == id)
    }

    /// The revision of a list as stored now (`None` when it is not in the listing).
    pub(crate) async fn fresh_list(&self, id: ObjectId) -> Option<crate::tasks_v1::Revision> {
        let all = self.dir.lists(Trash::Include).await.ok()?;
        all.into_iter().find(|l| l.id == id).map(|l| l.revision)
    }

    /// The summary with `id` among the answers of an arbitrary query.
    pub(crate) async fn summary_in(&self, q: &TaskQuery, id: ObjectId) -> Option<TaskSummary> {
        self.all(q).await.into_iter().find(|s| s.id == id)
    }

    pub(crate) async fn set_progress(&mut self, t: &Task, to: Progress) -> Option<Task> {
        let r = self
            .dir
            .progress(
                t.id,
                t.revision,
                to,
                crate::tasks_v1::Children::Leave,
                crate::tasks_v1::SeriesScope::Whole,
            )
            .await;
        self.ensure(r.is_ok(), "setup: progress");
        r.ok()
    }
}

/// A draft with the title and list only in the draft, the rest in `rest`.
pub(crate) fn draft(list: ObjectId, title: &str, rest: TaskPatch) -> TaskDraft {
    TaskDraft {
        list,
        uid: None,
        title: title.to_owned(),
        progress: Progress::NeedsAction,
        assignees: Vec::new(),
        checklist: Vec::new(),
        rest,
    }
}

pub(crate) fn is_invalid(e: &TaskError) -> bool {
    matches!(e, TaskError::Common(Error::Invalid(_)))
}
pub(crate) fn is_not_found(e: &TaskError) -> bool {
    matches!(e, TaskError::Common(Error::NotFound(_)))
}
pub(crate) fn is_changed(e: &TaskError) -> bool {
    matches!(e, TaskError::Common(Error::Changed { .. }))
}
pub(crate) fn is_forbidden(e: &TaskError) -> bool {
    matches!(e, TaskError::Common(Error::Forbidden { .. }))
}
pub(crate) fn is_unsupported(e: &TaskError) -> bool {
    matches!(e, TaskError::Common(Error::Unsupported { .. }))
}
/// A zone nobody knows is `Invalid` with the code of the zone check (`zone.bad`), as in `appointments_v1`.
pub(crate) fn is_unknown_zone(e: &TaskError) -> bool {
    matches!(e, TaskError::Common(Error::Invalid(p)) if p.iter().any(|x| x.reason == "zone.bad"))
}
pub(crate) fn is_cycle(e: &TaskError) -> bool {
    matches!(e, TaskError::Cycle(_))
}
