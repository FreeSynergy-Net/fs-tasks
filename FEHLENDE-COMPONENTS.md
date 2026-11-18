# Missing components for the fs-tasks forms descriptions

Source of this list (slot C of the forms and scaffold priority): the descriptions in `forms/` against the block contract
0.4 of fs-components main `8ede48561d736104bd548ac504cfd88f83e6f018` (`contract/blocks/*.toml`). Nothing was invented: a block missing
from the catalog is named here with a description and the nearest existing block. "Provider" = the tasks program (field or grouping
names come from its `TypeCatalog`, which does not exist yet).

## Priority order for slot C (set by the strategy, 18.11.): field blocks first, then board slots, then the rest

### A. Field blocks (one block per basic type; roles input, display, compact; contract `fields.toml` has only text, email, name, phone, address, date)

| # | Component missing | Description | Used for (old id) | Nearest existing | Standards of the component |
|---|---|---|---|---|---|
| 1 | `field-datetime` | Date and time with zone, four roles | due F-069, start F-070 | `field-date` | RFC 8984 (JSCalendar) LocalDateTime / UTCDateTime; time rule UTC + zone |
| 2 | `field-choice` | One of a list, the list shown | priority F-073, status F-064 | `segmented` (not a field block) | RFC 8984 `priority` (0 to 9) and `progress` values; open for other lists |
| 3 | `field-bool` | Yes or no as a field | sensitive F-078 | `switch`, `checkbox` | open (RFC 8984 `privacy` is a choice, not a bool) |
| 4 | `field-multiline` | Several lines of text | description F-061 | `textbox` (area), `text-view` | RFC 8984 `description` (+ `descriptionContentType`); markup format open |
| 5 | `field-keywords` | Set of labels, chips, comma entry | labels F-063 | `tag` (one pill) | RFC 8984 `keywords` |
| 6 | `field-reference` (compact, display, cell; only input exists) | Reference to a person or task | participants F-066, related tasks | `person-picker` (input), `avatar-group`, `participant` | RFC 8984 `participants`, `relatedTo`; persons as IRI reference (ADR 0085) |
| 7 | `field-checklist` | Ticked items inside a task | checklist F-075 | `checkbox` per item | open (not in RFC 8984) |
| 8 | `field-duration` | Length of time | estimated duration F-074 | none | RFC 8984 `estimatedDuration` (ISO 8601 duration) |
| 9 | `field-recurrence` | Repeat rule editor and one-line display | repeats F-072 | none | RFC 8984 `recurrenceRules` (RFC 5545 RRULE) |
| 10 | `field-timezone` | Zone name with search | zone F-069 | none | RFC 8984 `timeZone` (IANA zone names) |
| 11 | `field-number` | Number with bounds | percent done F-065 | `textbox`, `progressbar` (display only) | RFC 8984 `percentComplete` (0 to 100); other numbers open |
| 12 | `field-link` | Address with the per-use scheme list (ADR 0096 point 11) | links F-076 | `link` | RFC 8984 `links`; RFC 3986 scheme names |
| 13 | `field-color` | Colour pick | colour F-077 | none | RFC 8984 `color` (CSS colour) |

Without these, `choose(field, role)` has no block for the task form (`edit` page of `forms/list.toml`) and the board card facts (due,
priority, participants, keywords). Not an error of the descriptions: the bindings name the fields, the catalog must answer.

### B. Board slots and the filter bar

| # | Component slot missing | Description | Block | Nearest | Standards of the component |
|---|---|---|---|---|---|
| 14 | `board`: `card-key` (value, text) | Identifier "FS-n" on the card (F-007) | `board` | `card-facts` (limit 4, full) | open (the key is a provider field) |
| 15 | `board`: `card-mark` (derived, display) | Mark "why it waits" for a blocked card (F-007, F-008) | `board` | `card-lock` (claims only) | open (RFC 8984 has relations, no wait reason) |
| 16 | `task-row`: inline open | Open and close of a row in the day list (F-011) | `task-row` | `board-detail` (side frame) | open |
| 17 | Filter bar (slot kind `condition`) | Inputs bound to a field (label, project, status) | new | `searchbox`, `tag`, `tablist` | open |

### C. Blocks whose slots are not declared (`accepts = []`), placed without bindings

| # | Component slot missing | Description | Block | Used in |
|---|---|---|---|---|
| 18 | `grid`: `columns` (field, role cell), `rows`, `row-menu` | Columns Title, Due, Priority, Status | `grid` | `forms/list.toml` page `list` |
| 19 | `task-row`: `rows`, `done` (command), `title`, `facts`, `labels`, `group` (grouping) | One task as a line; groups TODAY, READY, BLOCKED | `task-row` | `forms/day.toml` |
| 20 | `quick-add`: `command`, `hint` | Entry row for the next task | `quick-add` | `forms/day.toml` |
| 21 | `sidebar`: `items`, `selected` (chosen list as source scope) | Projects as headings, labels | `sidebar` | `forms/board.toml`, `forms/day.toml` F-010 |
| 22 | `partial-notice`: source and reason | Note above a partial result | `partial-notice` | all |

Frontend switch (old menu Frontend): not a component; host or format (forms/README.md question 2).

Provider names unproven (no `TypeCatalog`): fields `progress`, `priority`, `due`, `participants`, `keywords`, `note`, `description`,
`timeZone`, `percentComplete`, `recurrenceRules`, `relatedTo`, `sensitive`, `progressExtended`; groupings `board`, `priority`, `standing`;
orders `rank`, `priority`, `due`; commands `create`, `claim`, `release`, `note`, `delete`, `move`, `done`, `reopen`.
