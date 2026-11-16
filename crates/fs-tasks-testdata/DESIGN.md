# fs-tasks-testdata: design

Step K0 of the pilot `programm-grundgeruest` for the program fs-tasks (plan: `.scratch/pilot-tasks/PLAN.md`, sections 5, 6
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
* **[`sample`]**: about 45 tasks in 4 lists, fixed ids and uids (`urn:fs-testdata:<slug>`), "today" is 2026-11-16.
* **[`FakeExchange`]**: the boundary `TaskExchange`, honest: no files, so `formats()` says `No` and import/export are
  `Unsupported` (the trait default). The real VTODO and JSCalendar adapters are step K3.

## The lock

`src/tasks_v1/` is the trait folder `tasks_v1`, **byte for byte** the locked one:

| | |
|---|---|
| SHA-256 (`tools/hash-ordner.sh`: sorted relative paths and contents, `LC_ALL=C`) | `66c7cf132f53fb0bb0b484494b72f308bdfb7e012ebad1e47ae7f2554aaed085` |
| lock file | `.scratch/sperren/tasks_v1.sha256` (locked 2026-11-16 by the strategy after the Opus review `review-trait-3-opus.md`, JA) |
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

The list is [`DECISIONS`] (ten entries, also tested). The important ones: one write = one second on the fake's clock and
the trash lasts 30 days; only daily, weekly, monthly and yearly rules (interval, `Never`/`Count`/`Until`) are expanded
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

## Layout

`state.rs` (the value behind the lock; every rule is a method of it, so one call is one transaction) with `ops_*.rs`
(commands), `select.rs` (reads), `ready.rs` (dependencies), `series.rs` and `scope.rs` (series), `ranking.rs`,
`patch.rs` (checks), `view.rs` (`TaskSummary`), `clock.rs` (time arithmetic); `fake.rs` and `fake_impl.rs` (the trait
as a thin facade); `sample*.rs`; `conformance/`. Every file has at most 300 lines and a `Pattern:` line.

## Not in this step

`FailingTasks`, `SlowTasks` (states Z2/Z3 of the frontend) and `FakePeople` (a `PersonDirectory` fake) of PLAN section 6
are not written yet; `FakeExchange` has no real format adapter (K3); `ThisAndFollowing` for series with `by_*` parts and
the Loro-style concurrent spread of two replicas are out of scope (v2, PLAN section 11).
