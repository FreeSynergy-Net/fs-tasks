//! The tables of step 5 (run 2a rest 2): the dependencies of a task, its alerts and links, and the derived day columns of
//! its due and start values. Every field has a typed column (no JSON, no blob).
//!
//! Pattern: Value Object (const table specifications, one source for the `CREATE TABLE` text and the registry).

use fs_objects::v5::spec::{
    Col,
    Logical::{Bool, Date, Enum, Int, Iri, Ref, Text, Utc},
    Table,
};

/// The words of a dependency kind (RFC 9253 `RELTYPE`, lower case).
pub const DEP_KIND: &[&str] = &[
    "finish-to-start",
    "finish-to-finish",
    "start-to-start",
    "start-to-finish",
    "depends-on",
];
/// What a trigger of an alert is.
pub const TRIGGER_KIND: &[&str] = &["offset", "absolute"];
/// What an offset trigger counts from.
pub const RELATIVE_TO: &[&str] = &["start", "end"];
/// What an alert does.
pub const ALERT_ACTION: &[&str] = &["display", "email"];

/// One dependency of a task: it holds the tie to `other_uid` (the other task is named by its `uid`, also a foreign one,
/// so no foreign key). `pos` is the order the holder was given them in. The gap is three columns, all empty or all set,
/// and empty for `depends-on` (RFC 9253).
pub const DEPENDENCY: Table = Table {
    name: "tasks_dependency",
    cols: &[
        Col::req("task", Ref),
        Col::req("pos", Int),
        Col::req("other_uid", Text),
        Col::req("kind", Enum(DEP_KIND)),
        Col::opt("gap_negative", Bool),
        Col::opt("gap_days", Int),
        Col::opt("gap_seconds", Int),
    ],
    constraints: &[
        "PRIMARY KEY (task, other_uid, kind)",
        "FOREIGN KEY (task) REFERENCES tasks_task(id) ON DELETE CASCADE",
        "CHECK (pos >= 0)",
        "CHECK (other_uid <> '')",
        "CHECK ((gap_negative IS NULL) = (gap_days IS NULL) AND (gap_days IS NULL) = (gap_seconds IS NULL))",
        "CHECK (gap_days IS NULL OR gap_days >= 0 AND gap_seconds >= 0)",
        "CHECK (kind <> 'depends-on' OR gap_days IS NULL)",
    ],
    indexes: &[
        "CREATE INDEX IF NOT EXISTS tasks_dependency_other ON tasks_dependency (other_uid)",
        "CREATE INDEX IF NOT EXISTS tasks_dependency_order ON tasks_dependency (task, pos)",
    ],
};

/// One alert of a task (`alerts`, a map by `key`; `pos` keeps the order given). The trigger is a group: an offset
/// (`offset_seconds`, `relative_to`) or an instant (`trigger_at`).
pub const ALERT: Table = Table {
    name: "tasks_alert",
    cols: &[
        Col::req("task", Ref),
        Col::req("key", Text),
        Col::req("pos", Int),
        Col::req("trigger_kind", Enum(TRIGGER_KIND)),
        Col::opt("offset_seconds", Int),
        Col::opt("relative_to", Enum(RELATIVE_TO)),
        Col::opt("trigger_at", Utc),
        Col::req("alert_action", Enum(ALERT_ACTION)),
        Col::opt("acknowledged", Utc),
    ],
    constraints: &[
        "PRIMARY KEY (task, key)",
        "FOREIGN KEY (task) REFERENCES tasks_task(id) ON DELETE CASCADE",
        "CHECK (pos >= 0)",
        "CHECK (key <> '')",
        "CHECK (trigger_kind = 'offset' AND offset_seconds IS NOT NULL AND relative_to IS NOT NULL AND trigger_at IS NULL \
         OR trigger_kind = 'absolute' AND trigger_at IS NOT NULL AND offset_seconds IS NULL AND relative_to IS NULL)",
    ],
    indexes: &["CREATE INDEX IF NOT EXISTS tasks_alert_order ON tasks_alert (task, pos)"],
};

/// One link of a task (`links`, a map by `key`; `pos` keeps the order): an address (`href`) or a file of ours (`file_*`: the
/// reference, its checksum as 64 hex digits, its media type), exactly one of them.
pub const LINK: Table = Table {
    name: "tasks_link",
    cols: &[
        Col::req("task", Ref),
        Col::req("key", Text),
        Col::req("pos", Int),
        Col::opt("href", Text),
        Col::opt("file_iri", Iri),
        Col::opt("file_sha256", Text),
        Col::opt("file_media", Text),
        Col::opt("content_type", Text),
        Col::opt("size", Int),
        Col::opt("rel", Text),
        Col::opt("title", Text),
    ],
    constraints: &[
        "PRIMARY KEY (task, key)",
        "FOREIGN KEY (task) REFERENCES tasks_task(id) ON DELETE CASCADE",
        "CHECK (pos >= 0)",
        "CHECK (key <> '')",
        "CHECK ((href IS NULL) <> (file_iri IS NULL))",
        "CHECK ((file_iri IS NULL) = (file_sha256 IS NULL) AND (file_iri IS NULL) = (file_media IS NULL))",
        "CHECK (size IS NULL OR size >= 0)",
    ],
    indexes: &["CREATE INDEX IF NOT EXISTS tasks_link_order ON tasks_link (task, pos)"],
};

/// The day columns of a task, derived from its due and start values on every write (never accepted from a caller): a
/// plain date and a floating time count by their own day (`*_day`), a timed value by its instant (`*_utc`), because its day
/// depends on the zone the asker views it in. The day range of `list` is decided on these (see `day.rs`).
pub const DAY: Table = Table {
    name: "tasks_day",
    cols: &[
        Col::req("task", Ref),
        Col::derived("due_day", Date, true),
        Col::derived("due_utc", Utc, true),
        Col::derived("start_day", Date, true),
        Col::derived("start_utc", Utc, true),
    ],
    constraints: &[
        "PRIMARY KEY (task)",
        "FOREIGN KEY (task) REFERENCES tasks_task(id) ON DELETE CASCADE",
        "CHECK (due_day IS NULL OR due_utc IS NULL)",
        "CHECK (start_day IS NULL OR start_utc IS NULL)",
    ],
    indexes: &[],
};

/// The tables of step 5.
pub const STEP_5: [Table; 4] = [DEPENDENCY, ALERT, LINK, DAY];
