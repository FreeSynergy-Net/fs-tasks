//! The domain tables `tasks_*` (PLAN section 3.1): lists, tasks, labels and the grants of lists. Every field has a typed
//! column, no JSON and no blob; the checklist, assignees, alerts, links, rules, dependencies and the claim are the
//! tables of run 2 (named in `stand.md`; the dependencies, alerts, links and day columns are in `tables_more`). `created` and `updated` are not repeated here: the envelope (`objects`)
//! has them (`published`, `updated`), one source.
//!
//! Pattern: Value Object (const table specifications, one source for the `CREATE TABLE` text and the registry).

use fs_objects::v5::spec::{
    Col,
    Logical::{Bool, Date, Enum, Int, Iri, Local, Ref, Text, Utc, Zone},
    Table,
};
use fs_objects::v5::tables::ART;

/// The prefix of the family (the layer refuses a table that does not start with it).
pub const PREFIX: &str = "tasks";
/// `description_type`.
pub const TEXT_TYPE: &[&str] = &["plain", "html"];
/// The form of a due value (`Due`): a plain date, a time in a zone, a floating time.
pub const DUE_KIND: &[&str] = &["date", "timed", "floating"];
/// `progress` (JSCalendar words).
pub const PROGRESS: &[&str] = &[
    "needs-action",
    "in-process",
    "completed",
    "failed",
    "cancelled",
];

/// One list; the envelope holds revision, trash and owner-as-author.
pub const LIST: Table = Table {
    name: "tasks_list",
    cols: &[
        Col::req("id", Ref),
        Col::req("name", Text),
        Col::opt("color", Text),
        Col::req("owner_id", Iri),
        Col::req("owner_art", Enum(ART)),
        Col::opt("owner_abzug", Ref),
    ],
    constraints: &[
        "PRIMARY KEY (id)",
        "FOREIGN KEY (id) REFERENCES objects(id) ON DELETE CASCADE",
    ],
    indexes: &["CREATE INDEX IF NOT EXISTS tasks_list_owner ON tasks_list (owner_id)"],
};

/// One task. `due_*` and `start_*` are the same group of columns: the kind decides which of `date`, `local` and `zone`
/// are set. `deleted_with` names the task whose deletion took this one along (a subtask); a task of a deleted LIST
/// carries nothing, its trash state is the list's.
pub const TASK: Table = Table {
    name: "tasks_task",
    cols: &[
        Col::req("id", Ref),
        Col::req("list", Ref),
        Col::req("uid", Text),
        Col::opt("parent", Ref),
        Col::opt("deleted_with", Ref),
        Col::req("sequence", Int),
        Col::req("title", Text),
        Col::req("description", Text),
        Col::req("description_type", Enum(TEXT_TYPE)),
        Col::opt("locale", Text),
        Col::opt("due_kind", Enum(DUE_KIND)),
        Col::opt("due_date", Date),
        Col::opt("due_local", Local),
        Col::opt("due_zone", Zone),
        Col::req("due_zone_assumed", Bool),
        Col::opt("start_kind", Enum(DUE_KIND)),
        Col::opt("start_date", Date),
        Col::opt("start_local", Local),
        Col::opt("start_zone", Zone),
        Col::req("start_zone_assumed", Bool),
        Col::opt("estimated_days", Int),
        Col::opt("estimated_seconds", Int),
        Col::req("progress", Enum(PROGRESS)),
        Col::opt("progress_updated", Utc),
        Col::opt("percent_complete", Int),
        Col::req("priority", Int),
        Col::opt("color", Text),
        Col::req("rank", Text),
    ],
    constraints: &[
        "PRIMARY KEY (id)",
        "FOREIGN KEY (id) REFERENCES objects(id) ON DELETE CASCADE",
        "FOREIGN KEY (list) REFERENCES tasks_list(id)",
        "FOREIGN KEY (parent) REFERENCES tasks_task(id) ON DELETE CASCADE",
        "FOREIGN KEY (deleted_with) REFERENCES objects(id)",
        "UNIQUE (uid)",
        "CHECK (priority BETWEEN 0 AND 9)",
        "CHECK (percent_complete IS NULL OR percent_complete BETWEEN 0 AND 100)",
        "CHECK (parent IS NULL OR parent <> id)",
        "CHECK ((estimated_days IS NULL) = (estimated_seconds IS NULL))",
        "CHECK (due_kind IS NULL AND due_date IS NULL AND due_local IS NULL AND due_zone IS NULL AND due_zone_assumed = 0 \
         OR due_kind = 'date' AND due_date IS NOT NULL AND due_local IS NULL AND due_zone IS NULL AND due_zone_assumed = 0 \
         OR due_kind = 'timed' AND due_local IS NOT NULL AND due_date IS NULL AND due_zone IS NOT NULL \
         OR due_kind = 'floating' AND due_local IS NOT NULL AND due_date IS NULL AND due_zone IS NULL AND due_zone_assumed = 0)",
        "CHECK (start_kind IS NULL AND start_date IS NULL AND start_local IS NULL AND start_zone IS NULL AND start_zone_assumed = 0 \
         OR start_kind = 'date' AND start_date IS NOT NULL AND start_local IS NULL AND start_zone IS NULL AND start_zone_assumed = 0 \
         OR start_kind = 'timed' AND start_local IS NOT NULL AND start_date IS NULL AND start_zone IS NOT NULL \
         OR start_kind = 'floating' AND start_local IS NOT NULL AND start_date IS NULL AND start_zone IS NULL AND start_zone_assumed = 0)",
    ],
    indexes: &[
        "CREATE INDEX IF NOT EXISTS tasks_task_column ON tasks_task (list, progress, rank)",
        "CREATE INDEX IF NOT EXISTS tasks_task_parent ON tasks_task (parent)",
        "CREATE INDEX IF NOT EXISTS tasks_task_with ON tasks_task (deleted_with)",
    ],
};

/// The labels of a task (`keywords`, a set).
pub const LABEL: Table = Table {
    name: "tasks_label",
    cols: &[Col::req("task", Ref), Col::req("value", Text)],
    constraints: &[
        "PRIMARY KEY (task, value)",
        "FOREIGN KEY (task) REFERENCES tasks_task(id) ON DELETE CASCADE",
    ],
    indexes: &["CREATE INDEX IF NOT EXISTS tasks_label_value ON tasks_label (value)"],
};

/// Who may see a list besides its owner, or sees less than the owner would: one row per list and person (step 2: run
/// 2a). A grant row REPLACES the level of the owner (the owner made `Read` only reads); without a row the owner has
/// `Manage` and nobody else sees the list. Groups come with run 2b (`GroupMembers`).
pub const GRANT: Table = Table {
    name: "tasks_grant",
    cols: &[
        Col::req("list", Ref),
        Col::req("who_id", Iri),
        Col::req("who_art", Enum(ART)),
        Col::opt("who_abzug", Ref),
        Col::req("level", Enum(crate::access::LEVELS)),
    ],
    constraints: &[
        "PRIMARY KEY (list, who_id)",
        "FOREIGN KEY (list) REFERENCES tasks_list(id) ON DELETE CASCADE",
    ],
    indexes: &[],
};

/// The checklist of a task: light items to tick (no subtasks). `key` is the stable name of an item inside its task (it
/// stays when the item is renamed or moved), `rank` the hand order inside the task. `done_*` is who ticked and when:
/// all empty (open) or all set.
pub const CHECKLIST: Table = Table {
    name: "tasks_checklist",
    cols: &[
        Col::req("task", Ref),
        Col::req("key", Text),
        Col::req("title", Text),
        Col::req("rank", Text),
        Col::opt("done_by_id", Iri),
        Col::opt("done_by_art", Enum(ART)),
        Col::opt("done_by_abzug", Ref),
        Col::opt("done_at", Utc),
    ],
    constraints: &[
        "PRIMARY KEY (task, key)",
        "FOREIGN KEY (task) REFERENCES tasks_task(id) ON DELETE CASCADE",
        "CHECK (done_by_id IS NULL AND done_by_art IS NULL AND done_by_abzug IS NULL AND done_at IS NULL \
         OR done_by_id IS NOT NULL AND done_by_art IS NOT NULL AND done_at IS NOT NULL)",
    ],
    indexes: &["CREATE INDEX IF NOT EXISTS tasks_checklist_order ON tasks_checklist (task, rank)"],
};

/// The highest item number ever given in the checklist of a task: a key is never given twice, not even after the item
/// that held it was removed (step 4).
pub const CHECKLIST_SEQ: Table = Table {
    name: "tasks_checklist_seq",
    cols: &[Col::req("task", Ref), Col::req("last", Int)],
    constraints: &[
        "PRIMARY KEY (task)",
        "FOREIGN KEY (task) REFERENCES tasks_task(id) ON DELETE CASCADE",
        "CHECK (last >= 0)",
    ],
    indexes: &[],
};

/// The tables of step 1 (run 1).
pub const STEP_1: [Table; 3] = [LIST, TASK, LABEL];
/// The tables of step 2 (run 2a).
pub const STEP_2: [Table; 1] = [GRANT];
/// The tables of step 3 (run 2a, the checklist).
pub const STEP_3: [Table; 1] = [CHECKLIST];
/// The tables of step 4 (run 2a rest, the key counter of the checklist).
pub const STEP_4: [Table; 1] = [CHECKLIST_SEQ];
/// All tables of the domain, in creation order (a table comes after the ones it points to).
pub const ALL: [Table; 10] = [
    LIST,
    TASK,
    LABEL,
    GRANT,
    CHECKLIST,
    CHECKLIST_SEQ,
    crate::tables_more::DEPENDENCY,
    crate::tables_more::ALERT,
    crate::tables_more::LINK,
    crate::tables_more::DAY,
];
