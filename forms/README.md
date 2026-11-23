# fs-tasks: forms descriptions of the frontends (prescribed, not yet imported)

Rule (user rules, ADR 0096, entscheid 45/49): a program is a blueprint plus a forms description. The frontends of fs-tasks are
**descriptions**, never a hand-written frontend crate: names of components, slots, fields, commands and MF2 text keys, no code,
no expression. These files are the **import form**; the description lives as data in `forms.db` (no TOML as storage or config).
This directory prescribes them from the existing views; the existing code (`crates/fs-tasks-app`) is untouched.

| File | Frontend id | Old frontend | Pages |
|---|---|---|---|
| `board.toml` | `board` (start frontend) | Board | `board` (side list, board, side panel, notice, empty) |
| `list.toml` | `classic` | Classic | `list` (list and status view), `edit` (task form, 13 rows) |
| `day.toml` | `day` | Day list | `day` (side list, rows, quick add) |

Format: Format 1 of `fs-forms-exchange` (ids derived; export writes Format 2). Keys, words and rules are those of the reader on the
probe branch `unreviewed/probe-forms-f1b-5` (ccfc58f0) of fs-forms; **fs-forms main (cf8502cc) has no exchange crate yet**, so no CI
of any repo can import these files today. Components: block contract 0.4 of fs-components main 8ede4856 (`contract/blocks/*.toml`).
Location (decided by the strategy): `forms/` in the repo root next to `scaffold.toml`; the program IS the description.

## Coverage of the old list (LISTE-fs-tasks-funktionen-vorher-2026-11-18, 19 views)

| Old | What | Where | State |
|---|---|---|---|
| F-001 | Classic window | `list.toml` frame `workbench` | placed |
| F-002, F-003 | List, status view | `list.toml` page `list`, views `list`, `status` | grid placed WITHOUT column bindings (slot missing) |
| F-004 | Compact variant | host (no second view, no menu) | PLATZHALTER |
| F-005 | Task form | `list.toml` page `edit`: 13 `form-field` rows | bound; field blocks missing (see FEHLENDE-COMPONENTS.md) |
| F-006 | Board | `board.toml` page `board`, block `board`, 12 bindings | bound |
| F-007 | Card | `board` slots `card-title`, `card-facts` (4), `card-lock`, `card-menu` | key "FS-n" and wait mark missing (slot) |
| F-008 | Card side panel | `board.toml` place `detail`, block `board-detail`, 9 bindings | bound |
| F-009 | Entry line per column | `board` slot `quick-add` + action `quick-add` | bound |
| F-010 | Side list projects, labels | `board.toml`, `day.toml` block `sidebar` | placed WITHOUT bindings (slots missing) |
| F-011 | Day list | `day.toml`: `task-row`, grouping `standing`, `quick-add` | placed WITHOUT bindings (slots missing) |
| F-012 | Choose list, `--list` | `day.toml` view `today`, source scope `selected-list` | start list = open question 4 |
| F-013, F-015, F-016 | Trash, search, calendar | not in the old window (UNPROVEN) | PLATZHALTER.md |
| F-014 | Filters | filter tree not readable in this format cut | PLATZHALTER.md, open question 3 |
| F-017, F-018, F-019 | Problem window, notices, picture | host (`host-error-*`, `partial-notice`, `--snapshot`) | PLATZHALTER.md |

## Menus (old M-001..M-007)

| Old | Entry | New |
|---|---|---|
| M-001 | Task > New (`task-new`) | `list.toml` menu `menu-task` command `planning.create`, key `ctrl+n`; also board, day |
| M-002, M-003 | View > List, Status | `list.toml` menu `menu-view`, entries `view` `list`, `status` |
| M-004..M-006 | Frontend > Classic, Board, Day list | NOT expressible: the switch between descriptions is a host function (open question 2) |
| M-007 | Problem window Yes / No / Close | host (`confirm-dialog`, `host-error-*`) |
| new | Board: Task (create, delete; claim, release, note come later) and View (lanes, three orders) | `board.toml`, from the F1 sketch and the menu dossier; not in the old board |

## Commands to workflow blocks

A command of the program is a workflow block (rule "macros are workflows"; ADR 0087). A description names commands as
`source` + `command`; actions say the trigger. No `workflow` target is written: a workflow reference needs an id nobody has
(open question 2). Command names marked (?) are proposed; the tasks `TypeCatalog` does not exist yet.

| Old CLI | Command in the description | Used by |
|---|---|---|
| F-023 `add` | `create` | menu, quick-add, `board` slot `quick-add` |
| F-024 `done`, (tick) | `done` (?), `reopen` (?) | `day.toml` actions |
| F-025 `delete` | `delete` with `confirm` | board menu, card menu, classic action |
| F-028 `claim`, F-029 `release` | (none yet) | NOT in `board.toml`: the module `planning` has no such command (PLATZHALTER.md) |
| F-032 `note` | (none yet) | NOT in `board.toml`: no such command, no `note` member in the contract (PLATZHALTER.md) |
| (drag, Move to) | `move` (?) | `board` slot `move`, action `drop` |
| F-020 `capabilities`, F-021 `list`, F-022 `show`, F-030 `next`, F-031 `status`, F-033 `check`, F-026/F-027 `export`/`import`, F-034 `serve`, F-035 `app` | not frontend content: contract methods (the CLI is generated from the contract, ADR 0096 point 13) or the starter | none |

## Checked and not checked

Checked by lint (reading, no build, no cargo): TOML syntax; closed key sets per table; closed words (kinds, triggers, scope, confirm,
roles, `by`); menu pair rules (command needs command, view needs view); source numbers; block names, slot names, `nth` below the
slot `repeat`, required slots against `contract/blocks/*.toml`; every text key used has `de` and `en`. The lint (a small script) mirrors
`fs-forms-exchange/src/read.rs`, `fs-forms-layout/src/rules.rs`, `check.rs` and is **not** the real reader.
**Not checked by machine:** the real import (no build allowed, no exchange crate on main), MF2 syntax of the texts, field names against
the provider (no `TypeCatalog`), `choose(field, role)` against the catalog (not built), shortcuts against the keymap (`ctrl+n`, `delete`),
the unbelegt old behaviour of Classic (external crate). Hash `forms_bind_v1` was not recomputed (needs a build).

## Open questions

1. (answered: `forms/`, next to `scaffold.toml`.)
2. Frontend switch (Classic, Board, Day list) as a menu: host function or a menu kind in the format? Workflow targets: id format?
3. Filters (labels, project, status): the filter tree is not read in the format cut (`filter` is `key-unknown` today); extend the format?
4. Start list `--list` and `--plan`: host options or settings (`fs.shell.frontend`)?
5. One description with three pages or three frontends? Chosen: three (matches the three old frontends, the unique key is (extends, frontend)).
