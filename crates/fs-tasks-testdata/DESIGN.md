# fs-tasks-testdata: design

Steps K0, K0b and K0c (the delta of `tasks_v1`) of the pilot `programm-grundgeruest` for the program fs-tasks (plan: `.scratch/pilot-tasks/PLAN.md`, sections 5, 6
and 11). This crate is what every other part of the tasks program is tested against first: a fake of the locked trait
`Tasks`, rules any provider must pass, and one fixed set of sample data. `publish = false`; the core agent owns it, the
frontends only read it (PLAN section 6: "Frontend ändert `fs-tasks-testdata` nicht").

## Purpose

* **[`FakeTasks`]**: an in-memory provider of the trait `Tasks` (and `EmptyTasks`, the same without data). The frontend
  (fs-tasks-app) develops against it before `LocalTasks` exists; the one-line switch later is Fake to `LocalTasks`/`RemoteTasks`.
* **[`conformance`]**: `run_conformance`, `run_fixture_conformance`, `run_unsupported`. The rules of PLAN section 11 as
  code, written once and run against the fake now and against `LocalTasks` (K1/K2), `RemoteTasks` (K5) and the
  `compat` adapter later. Pattern after `fs-people-directory` (`ConformanceFixture` with mutators; a rule a fixture
  cannot set up is reported as skipped, never as passed).
* **[`sample`]**: about 100 tasks in 6 lists (one owned by a group), fixed ids and uids (`urn:fs-testdata:<slug>`), "today" is
  2026-11-16; group assignees, a claim, checklists (2 of 5 open), series from completion, the trash states `Manual` and
  `Held` ([`Sample::manual`], [`Sample::held`]).
* **[`FakeGroupMembers`]**: the groups the provider is built with (who is in which group, who manages it, who is an AI);
  the local narrow `GroupMembers` until fs-groups offers its trait at a tag. **[`HeldTasks`]**: calls stay open until `release`.
* **[`FakeExchange`]**: the boundary `TaskExchange`, honest: no files, so `formats()` says `No` and import/export are
  `Unsupported` (`not_possible`; the trait has no default methods). The real VTODO and JSCalendar adapters are step K3.

## The fixpoint

`src/tasks_v1/` is the trait folder `tasks_v1`, **byte for byte** the fixed one (work fixpoint, not a feature freeze):

| | |
|---|---|
| SHA-256 (`tools/hash-ordner.sh`: sorted relative paths and contents, `LC_ALL=C`) | `de4618e1719043728a982345f3a133b00f973b2855676243d62d47a301ed4d46` |
| lock file | `.scratch/sperren/tasks_v1.sha256` (fixed 2026-11-16 by the strategy after the Opus delta review `review-tasks-delta-2-opus.md`, JA) |
| pins | fs-types v0.0.22 = fs-interfaces `7743056396ffa80bc77d3af9d1946229dd1f6155`; fs-jscalendar v0.0.2 = `f3d651b73da2e440d24140df0bf4669a34c47cdc` |

Never edit the folder: a change is `tasks_v2` and a new plan (ADR 0045). The `mod` line in `src/lib.rs` carries the
lock comment, `#[rustfmt::skip]` (so `cargo fmt` cannot change a locked file) and the lint allows the folder needs
(they live outside it). The Cargo pins of this crate (`fs-types`, `fs-jscalendar`) are the pins of the lock; a changed
pin is a jump for the trait too (review-trait-opus.md, "Sperre mit `fs_jscalendar::...`"). Check before every commit:

```
tools/hash-ordner.sh crates/fs-tasks-testdata/src/tasks_v1   # must print the hash above
```

## Who expects what

* **fs-tasks-app** (frontends Einfach and Admin; views List, Heute, Logbuch, Wartet, Board, Papierkorb): all views are
  queries on `TaskQuery`: List = `lists` + `Order::Rank`; Heute = `due: to today, from open` + view zone; Logbuch =
  `progress: [Completed]` + `Order::Finished`; Wartet = `ready: Some(false)` (only with `dependency.evaluate`);
  Board = one query per column (`progress`) + `Order::Rank`, dragging = `progress` then `place`; Papierkorb =
  `Trash::Only`; filters = `labels`, `AssigneeFilter`. The sample has every state of these views (a series with
  exceptions, a cycle shown as `Conflict`, an unknown dependency, a cancelled predecessor, a claim, a trashed task).
* **fs-calendar-tasks** (adapter `compat`, pin fs-tasks v0.0.1): must pass `Due::Date` on as a plain date, must never
  map `NotEvaluated` to `Ready`, and gives imported tasks a rank with `Rank::spread`, not one by one. The fake is its
  reference for `Due` forms (`partnerbuero` is 08:00 in Tokyo, which is already the 17th in Berlin).
* **PersonDirectory** (fs-people, `suggest`): the provider gets it in its constructor; the picker in the frontend uses it.
  Not part of the trait. The fake only knows `PersonRef`; [`KNOWN`] names six invented people (all addresses at
  `example.org`, no real person or organisation) so a frontend can build a directory fake from them.
* **LocalTasks / RemoteTasks**: run `run_fixture_conformance` with a fixture of their own (clock, "I" card, account).

## What the fake decides (where the trait is silent)

The list is [`DECISIONS`] (fourteen entries, also tested). The important ones: one write = one second on the fake's clock and
the trash lasts 30 days (a setting: any days, or never); only daily, weekly, monthly and yearly rules (interval, `Never`/`Count`/`Until`) are expanded
and a rule outside that is `Invalid`, while a `Recurrence::Original` stays as text, shows its first occurrence and is
`Unsupported{task.edit}` for `This`/`ThisAndFollowing`; `AssigneeFilter::Me` meets the account AND the "I" card;
the `Move` entries of a re-spread are in `changes`, not in a task's `history`; a write that changes nothing writes nothing.

## Rules implemented (PLAN section 11 and the three trait reviews)

Rank: `create` and `progress` put a card on top; `place` writes only the moved task; 1000 appends at the top and at the
bottom stay short and write nobody else; equal neighbours and `Exhausted` spread the sequence once, in the same
transaction, without `Changed` for the caller (+1 revision and a `Move` in `changes` per rewritten task). Trash: `Trash`
`Exclude`/`Include`/`Only`, one deadline `restorable_until` for a deleted list and all its tasks (subtasks too), a task
of a deleted list is `Invalid` for `restore` and `NotFound` for a new task, a task deleted on its own stays deleted when
its list comes back, a subtask deleted with its parent comes back only with it, everything is gone after the deadline.
`expected`/`Changed{current}`; `readiness` with and without `dependency.evaluate` (`NotEvaluated`, `Done` first,
`Conflict`, `Unknown`, `DependencyCancelled`); `Cycle` with `uid`s also over `parent`; `changes` with `next` always set;
`labels`; series (`This`, `ThisAndFollowing` at the first occurrence = `Whole`, split with `First`/`Next`, trash of one
occurrence and of the following with `Changed` on a changed series, the last occurrence of a `Count` series finishes it);
`assign`/`unassign` idempotent; `asker` falls back to the account; `size = 0` is an empty page with `next`.

## The delta in the fake

Repeat from completion (`ops_repeat.rs`): the SAME task moves on, the finished occurrence stays as an exception; a series
from completion keeps to its form; the last completion finishes the task. Checklists (`ops_checklist.rs`): items of the
task, 100 at most, one log entry per change. Claim (`access.rs`, `ops_claim.rs`): rule over `memberships_of`, order
`NotFound`, `Forbidden`, `Claimed`, `claim_next`, AI rules and "STOP KI". Trash (`bin.rs`): `Restorable` (`Until`,
`Held`, `Manual`) computed from the clock and the keeps, expiry, `purge` with the cascade of a list and PLAN O18
(a subtask deleted on its own loses its `parent`, nothing is deleted through a `parent` row). Lists have an owner (a person
or a group) and an access. Rules: `conformance/rules_{checklist,repeat,repeat_more,claim,groups,purge}.rs`; every one has a
mutation of the fake (`mutation.rs`, `tests/mutants.rs`) that it must catch. `Options { heavy }` makes the rank rules write
6000 instead of 2000 tasks at each edge (night run).

## Layout

`state.rs` (the value behind the lock; every rule is a method of it, so one call is one transaction) with `ops_*.rs`
(commands), `select.rs` (reads), `ready.rs` (dependencies), `series.rs` and `scope.rs` (series), `ranking.rs`,
`patch.rs` (checks), `view.rs` (`TaskSummary`), `clock.rs` (time arithmetic); `fake.rs` and `fake_impl.rs` (the trait
as a thin facade); `sample*.rs`; `conformance/`. Every file has at most 300 lines and a `Pattern:` line.

## Not in this step

`FakeExchange` has no real format adapter (K3); `LocalTasks` (K1, fs-objects v0.0.3, typed tables) is not written; `ThisAndFollowing` for series with `by_*` parts and
the Loro-style concurrent spread of two replicas are out of scope (v2, PLAN section 11).

## Reading covers the log, the feed and `purge` (K0c fix)

Reading a list (`ListAccess::Read`) covers `list`, `get`, `history` and `changes`: whoever may not see the list gets
`NotFound` from `history` and `purge` (never `Ok`, never `Invalid` first) and finds nothing in `changes`; `restore` asks
`NotFound`, then `Forbidden`, then `Invalid` and checks the depth. `purge` of a parent detaches a kept subtask (and what
hangs below it) instead of deleting it, as `expire` does. What the feed holds of a list that is gone (purged, expired) is hidden from everybody, its
owner included: purge makes no feed entry, so hiding costs nobody anything, while showing it would give the AI
afterwards what it never saw (ids, the list, create/add/delete, times). `changes` fills its page from what follows
(a page is empty only at the end of the feed); whether the trait text should say that an empty page may not mean
the end is a question for the next round (the fake never gives such a page). Rules: `rules_leak.rs`, `rules_readonly.rs`, `rules_purge_kept.rs`, `rules_claim_order.rs`,
`rules_repeat_dates.rs`; mutations `LeakTrash`, `SeesAll`, `PurgeTakesKept`, `ClaimedFirst`, `ReleaseOpen`, `UndoAny`,
`NextIgnoresReady`, `RestoreInvalidFirst`, `RestoreNoDepth`. The fixture offers `read_only(who, list)` for the access `Read`.

## For K1 (`LocalTasks`, two runs, not one)

- `restorable_until` of fs-objects joins "kept" and "never automatically" to `Never` and reports a keep with an end as
  `Until`: do not use it for `Restorable`; compute it from `held_until`, `is_held` and `trash_auto` (Held before Until;
  Manual only when the automatic purge is off).
- `delete_final` follows every `parent` row: before it, in the same transaction, detach the subtasks deleted on their
  own and the kept ones; check a list purge for a kept task first.
- Open for the next trait text: N6 (finishing the same occurrence twice on one day: `Count` and the undo), and what
  deleting a future occurrence of a plan series does to its tick and claim (A4, A6 of the review).
