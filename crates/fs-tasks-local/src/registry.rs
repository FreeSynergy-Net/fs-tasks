//! The `Domain` of tasks for the shared layer (R1): prefix, tables, steps, dependent objects, channels.
//!
//! Pattern: Registry (a program reports its table family once; the layer creates the tables, writes them down and
//! runs the steps that are missing). One step, `Create`; a later change is a new step appended, with a fixture test
//! (ADR 0086). Run 2 appends the steps for the checklist, assignees, alerts, links, rules, dependencies and the claim.

use fs_objects::v5::Step;
use fs_objects::v5::envelope::Child;
use fs_objects::v5::registry::Domain;

use crate::day;
use crate::tables::{ALL, PREFIX, STEP_1, STEP_2, STEP_3, STEP_4};
use crate::tables_more::STEP_5;

/// The channels `object_origin.channel` may hold (the same as the calendar's: file, `CalDAV`, closed accounts, `iTIP`, hub,
/// subscription).
pub const CHANNELS: &[&str] = &[
    "file",
    "caldav",
    "google",
    "outlook",
    "itip",
    "hub",
    "subscription",
];

/// Dependent objects that leave with their parent at the final delete: the tasks of a list, and the subtasks of a task
/// (the layer follows them recursively and deletes the dependents first). A subtask deleted EARLIER on its own must NOT
/// go with its parent (PLAN O18): `trash_ops::detach` hangs it off before `delete_final`, in the same transaction.
const CHILDREN: &[Child] = &[
    Child {
        table: "tasks_task",
        parent: "list",
        object: "id",
    },
    Child {
        table: "tasks_task",
        parent: "parent",
        object: "id",
    },
];

/// The domain of tasks.
pub const DOMAIN: Domain = Domain {
    prefix: PREFIX,
    about: "tasks: lists, tasks, labels, grants, checklists, dependencies, alerts, links, day columns",
    tables: &ALL,
    channels: CHANNELS,
    children: CHILDREN,
    expiries: &[],
    steps: &[
        Step::Create(&STEP_1),
        Step::Create(&STEP_2),
        Step::Create(&STEP_3),
        Step::Create(&STEP_4),
        Step::Create(&STEP_5),
        Step::Code("tasks_day_fill", day::fill),
    ],
};
