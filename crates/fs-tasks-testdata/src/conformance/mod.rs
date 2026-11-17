//! Conformance rules of `tasks_v1`, runnable against any provider (the fake, `LocalTasks`, `RemoteTasks`).
//!
//! Pattern: Template method (a fixed list of rules, each one checked against the `Tasks` under test).
//!
//! `run_conformance` seeds its own data through `create_list` and `create` (every list is named `Zzconf ...`), so it
//! needs a provider with `list.create` and `task.create` and works on an empty or a filled provider. It is the rules
//! of PLAN section 11. A provider that cannot write runs `run_unsupported` instead. The rules that need a state the
//! trait cannot produce (a clock past the trash deadline, an "I" card) are in `run_fixture_conformance`; a fixture
//! that cannot offer a mutator keeps its default and the rules that need it are reported as skipped, not as passed.

mod cx;
mod fixture;
mod rules_assign;
mod rules_checklist;
mod rules_checklist_more;
mod rules_claim;
mod rules_claim_order;
mod rules_due;
mod rules_feed;
mod rules_groups;
mod rules_labels;
mod rules_leak;
mod rules_lists;
mod rules_off;
mod rules_parts;
mod rules_purge;
mod rules_purge_kept;
mod rules_query;
mod rules_rank;
mod rules_rank_top;
mod rules_readonly;
mod rules_ready;
mod rules_repeat;
mod rules_repeat_dates;
mod rules_repeat_ends;
mod rules_repeat_more;
mod rules_repeat_undo;
mod rules_series;
mod rules_subtasks;
mod rules_tasks;
mod rules_trash;
mod run;
mod run_unsupported;

pub use cx::Options;
pub use fixture::ConformanceFixture;
pub use run::{
    RankPart, Report, run_conformance, run_conformance_with, run_fixture_conformance,
    run_fixture_conformance_with, run_rank_conformance,
};
pub use run_unsupported::run_unsupported;
