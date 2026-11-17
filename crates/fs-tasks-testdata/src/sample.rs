//! The fixed sample: the lists and tasks every frontend picture and every provider test uses.
//!
//! Pattern: Builder of fixed data (no randomness, no clock): the same ids, uids and times on every run. "Today" is
//! 2026-11-16 (a Monday). The slug of a task is its name in tests; its `uid` is `urn:fs-testdata:<slug>`.

use crate::blank::blank_task;
use crate::clock::local;
use crate::fake::{FakeTasks, START_MS};
use crate::ids::{object_id, person};
use crate::sample_people::{KNOWN, Known};
use crate::tasks_v1::{
    Assignee, AssigneeStatus, Date, Dependency, DependencyKind, Due, ListAccess, ListSource,
    LocalDateTime, ObjectId, Override, OverrideChange, Progress, Recurrence, RecurrenceRule,
    Revision, Span, Task, TaskList, Utc, Zone,
};
use fs_types::BasicValue;

/// The whole sample.
#[derive(Clone, Debug)]
pub struct Sample {
    /// Six lists: "Umzug Hafenstraße" (1), "Website-Relaunch" (2), "Alltag" (3), "Verein Beispielhausen" (4),
    /// "Lange Liste" (5, one column of 40 tasks), "Vorstand Beispielhausen" (6, owned by the group "Team Vorstand").
    pub lists: Vec<TaskList>,
    /// Nine invented people: the acting person (1), colleagues (2 to 5), groups (6, 7), an AI (8), an AI group (9).
    pub people: Vec<Known>,
    /// The tasks with their slugs, in a fixed order.
    pub tasks: Vec<(&'static str, Task)>,
    /// A task in the trash (restorable for 20 days) and its slug.
    pub trashed: (&'static str, Task),
    /// A task in the trash with "never automatically" (`Restorable::Manual`) and its slug.
    pub manual: (&'static str, Task),
    /// A task in the trash that is kept without an end (`Restorable::Held`) and its slug.
    pub held: (&'static str, Task),
}

impl Sample {
    /// The task with this slug, `None` when the sample has none.
    #[must_use]
    pub fn try_get(&self, slug: &str) -> Option<&Task> {
        self.tasks.iter().find(|(s, _)| *s == slug).map(|(_, t)| t)
    }

    /// The task with this slug.
    ///
    /// # Panics
    /// If the slug is not in the sample (a typo in a test); [`Sample::try_get`] does not panic.
    #[must_use]
    pub fn get(&self, slug: &str) -> &Task {
        self.try_get(slug)
            .unwrap_or_else(|| panic!("no sample task `{slug}`"))
    }

    /// A [`FakeTasks`] loaded with the sample (the account is person 1).
    #[must_use]
    pub fn fake(&self) -> FakeTasks {
        let fake = FakeTasks::new();
        for l in &self.lists {
            fake.insert_list_raw(l.clone());
        }
        for (_, t) in &self.tasks {
            fake.insert_task_raw(t.clone());
        }
        fake.respread_ranks();
        fake.insert_deleted_raw(
            self.trashed.1.clone(),
            Utc::from_millis(START_MS + 20 * 86_400_000),
        );
        fake.insert_manual_raw(self.manual.1.clone());
        fake.insert_deleted_raw(
            self.held.1.clone(),
            Utc::from_millis(START_MS + 5 * 86_400_000),
        );
        fake.keep(self.held.1.id, None);
        fake
    }
}

/// Builder of one sample task.
pub(crate) struct B {
    pub t: Task,
    pub slug: &'static str,
}

fn at(days_before_today: i64) -> Utc {
    Utc::from_millis(START_MS - days_before_today * 86_400_000)
}

/// Start a task: `n` is its number (id `n`), `list` its list number.
pub(crate) fn task(n: u64, list: u64, slug: &'static str, title: &str) -> B {
    let t = blank_task(
        object_id(n),
        object_id(list),
        &format!("urn:fs-testdata:{slug}"),
        title,
        at(14),
    );
    B { t, slug }
}

/// # Panics
/// On a malformed literal of the sample.
pub(crate) fn date(text: &str) -> Date {
    Date::parse(text).expect("a date of the sample")
}

/// # Panics
/// On a malformed literal of the sample.
pub(crate) fn wall(text: &str) -> LocalDateTime {
    LocalDateTime::parse(text).expect("a local time of the sample")
}

impl B {
    pub(crate) fn due(mut self, day: &str) -> Self {
        self.t.due = Some(Due::Date(date(day)));
        self
    }
    pub(crate) fn due_at(mut self, local_time: &str, zone: &str) -> Self {
        let zone = Zone::iana(zone).expect("a zone of the sample");
        self.t.due = Some(Due::Timed {
            at: wall(local_time),
            zone,
            zone_assumed: false,
        });
        self
    }
    pub(crate) fn floating(mut self, local_time: &str) -> Self {
        self.t.due = Some(Due::Floating(wall(local_time)));
        self
    }
    pub(crate) fn start(mut self, day: &str) -> Self {
        self.t.start = Some(Due::Date(date(day)));
        self
    }
    pub(crate) fn prio(mut self, p: u8) -> Self {
        self.t.priority = p;
        self
    }
    pub(crate) fn labels(mut self, l: &[&str]) -> Self {
        self.t.labels = l.iter().map(|x| (*x).to_owned()).collect();
        self
    }
    pub(crate) fn desc(mut self, text: &str) -> Self {
        text.clone_into(&mut self.t.description);
        self
    }
    pub(crate) fn who(mut self, people: &[(u64, AssigneeStatus)]) -> Self {
        for (n, status) in people {
            let who = KNOWN
                .iter()
                .find(|k| k.number == *n)
                .map_or_else(|| person(*n), Known::person);
            self.t.assignees.push(Assignee {
                who,
                status: *status,
                assigned_by: Some(person(1)),
                assigned: Some(at(7)),
                comment: None,
            });
        }
        self
    }
    pub(crate) fn state(mut self, p: Progress, finished_days_ago: i64) -> Self {
        self.t.progress = p;
        if !matches!(p, Progress::NeedsAction | Progress::InProcess) {
            self.t.progress_updated = Some(at(finished_days_ago));
        }
        self
    }
    pub(crate) fn pct(mut self, p: u8) -> Self {
        self.t.percent_complete = Some(p);
        self
    }
    pub(crate) fn est(mut self, days: u32, seconds: u32) -> Self {
        self.t.estimated = Some(Span { days, seconds });
        self
    }
    pub(crate) fn parent(mut self, n: u64) -> Self {
        self.t.parent = Some(object_id(n));
        self
    }
    pub(crate) fn dep(mut self, slug: &str, kind: DependencyKind) -> Self {
        self.t.dependencies.push(Dependency {
            other_uid: format!("urn:fs-testdata:{slug}"),
            kind,
            gap: None,
        });
        self
    }
    pub(crate) fn series(mut self, rule: RecurrenceRule) -> Self {
        self.t.recurrence = vec![Recurrence::Rule(rule)];
        self
    }
    pub(crate) fn original_rule(mut self, text: &str) -> Self {
        self.t.recurrence = vec![Recurrence::Original(text.to_owned())];
        self
    }
    pub(crate) fn done_on(mut self, rid: &str) -> Self {
        let recurrence_id = wall(rid);
        let change = OverrideChange::Changed {
            patch: crate::tasks_v1::TaskPatch::default(),
            progress: Some(Progress::Completed),
            assignees: None,
        };
        self.t.overrides.push(Override {
            recurrence_id,
            change,
        });
        self
    }
    pub(crate) fn skip_on(mut self, rid: &str) -> Self {
        self.t.overrides.push(Override {
            recurrence_id: wall(rid),
            change: OverrideChange::Excluded,
        });
        self
    }
}

/// Number to id (helper for lists in the data files).
pub(crate) fn list(n: u64, name: &str, color: &str) -> TaskList {
    list_of(n, name, color, person(1))
}

/// A list of an owner (a person or a group).
pub(crate) fn list_of(
    n: u64,
    name: &str,
    color: &str,
    owner: crate::tasks_v1::PersonRef,
) -> TaskList {
    TaskList {
        id: object_id(n),
        name: name.to_owned(),
        color: Some(color.to_owned()),
        owner,
        revision: Revision(1),
        access: ListAccess::Manage,
        source: ListSource::Own,
        trash: None,
    }
}

/// The fixed sample.
#[must_use]
pub fn sample() -> Sample {
    let (tasks, trashed) = crate::sample_data::tasks();
    let (manual, held) = crate::sample_delta::trash_states();
    Sample {
        lists: vec![
            list(1, "Umzug Hafenstraße", "#3b82f6"),
            list(2, "Website-Relaunch", "#10b981"),
            list(3, "Alltag", "#f59e0b"),
            list(4, "Verein Beispielhausen", "#8b5cf6"),
            list(5, "Lange Liste", "#ef4444"),
            list_of(6, "Vorstand Beispielhausen", "#06b6d4", KNOWN[5].person()),
        ],
        people: KNOWN.to_vec(),
        tasks,
        trashed,
        manual,
        held,
    }
}

/// The local date-time of a literal as `Option` (for tests that build occurrences).
#[must_use]
pub fn occurrence(text: &str) -> Option<LocalDateTime> {
    local(chrono::NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S").ok()?)
}

/// The id of a sample number (task `n` has id `n`).
#[must_use]
pub fn id_of(n: u64) -> ObjectId {
    object_id(n)
}
