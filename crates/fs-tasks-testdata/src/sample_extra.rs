//! The additions of PLAN section 10: a task in New York (A8), priority 9 with a reminder and a link, a time whose zone was
//! assumed (A16), and one column of 40 tasks (list 5, for the board and the long list).
//!
//! Pattern: Fixed data (one function, one line of builder calls per task); the builder methods are `impl B` here.

use crate::sample::{B, task, wall};
use crate::tasks_v1::{Alert, AlertAction, Due, Link, RelativeTo, Task, Trigger, Zone};

type Pair = (&'static str, Task);

impl B {
    /// A time whose zone nobody said: imported, `zone_assumed = true` (A16).
    pub(crate) fn due_assumed(mut self, local_time: &str, zone: &str) -> Self {
        let zone = Zone::iana(zone).expect("a zone of the sample");
        self.t.due = Some(Due::Timed {
            at: wall(local_time),
            zone,
            zone_assumed: true,
        });
        self
    }
    /// A reminder `seconds` before the due value.
    pub(crate) fn alert(mut self, seconds: i64) -> Self {
        self.t.alerts.push(Alert {
            key: format!("alert-{}", self.t.alerts.len() + 1),
            trigger: Trigger::Offset {
                seconds: -seconds,
                relative_to: RelativeTo::End,
            },
            action: AlertAction::Display,
            acknowledged: None,
        });
        self
    }
    /// A link to an address at `example.org`.
    pub(crate) fn link(mut self, path: &str, title: &str) -> Self {
        self.t.links.push(Link {
            key: format!("link-{}", self.t.links.len() + 1),
            href: Some(format!("https://example.org/{path}")),
            file: None,
            content_type: None,
            size: None,
            rel: Some("related".to_owned()),
            title: Some(title.to_owned()),
        });
        self
    }
}

/// The slugs of the 40 tasks of list 5 (ids 501 to 540).
const COLUMN: [&str; 40] = [
    "brett-01", "brett-02", "brett-03", "brett-04", "brett-05", "brett-06", "brett-07", "brett-08",
    "brett-09", "brett-10", "brett-11", "brett-12", "brett-13", "brett-14", "brett-15", "brett-16",
    "brett-17", "brett-18", "brett-19", "brett-20", "brett-21", "brett-22", "brett-23", "brett-24",
    "brett-25", "brett-26", "brett-27", "brett-28", "brett-29", "brett-30", "brett-31", "brett-32",
    "brett-33", "brett-34", "brett-35", "brett-36", "brett-37", "brett-38", "brett-39", "brett-40",
];

/// Tasks 361 to 363 (list 3) and the column (list 5).
pub(crate) fn more(v: &mut Vec<Pair>) {
    let extra = [
        task(361, 3, "ny-abstimmung", "Abstimmung mit New York")
            .due_at("2026-11-16T15:00:00", "America/New_York")
            .prio(3),
        task(362, 3, "reisepass", "Reisepass verlängern")
            .due("2026-11-20")
            .prio(9)
            .alert(86_400)
            .link("reisepass", "Antrag beim Amt"),
        task(363, 3, "import-ohne-zone", "Importierte Zeit ohne Zone")
            .due_assumed("2026-11-17T10:00:00", "Europe/Berlin")
            .labels(&["import"]),
    ];
    for b in extra {
        v.push((b.slug, b.t));
    }
    for (i, slug) in COLUMN.iter().enumerate() {
        let b = task(
            501 + i as u64,
            5,
            slug,
            &format!("Aufgabe {} der langen Spalte", i + 1),
        );
        v.push((b.slug, b.t));
    }
}
