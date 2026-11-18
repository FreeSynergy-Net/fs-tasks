# Missing components for the fs-tasks forms descriptions

Source of this list (slot C of the forms and scaffold priority): the descriptions in `packaging/layout/` against the block contract
0.4 of fs-components main `8ede48561d736104bd548ac504cfd88f83e6f018` (`contract/blocks/*.toml`). Nothing was invented: a block missing
from the catalog is named here with a description and the nearest existing block. "Provider" = the tasks program (field or grouping
names come from its `TypeCatalog`, which does not exist yet).

## A. Field blocks (one block per basic type, roles input, display, compact; contract `fields.toml` has only text, email, name, phone, address, date)

| Component missing | Description | Used for (old id) | Nearest existing |
|---|---|---|---|
| `field-datetime` | Date and time with zone, four roles | due F-069, start F-070 | `field-date` (date only) |
| `field-choice` | One of a list with the list shown (priority 0 to 9 or empty, progress states) | priority F-073, status F-064 | `segmented` (not a field block) |
| `field-number` | Number with bounds | percent done F-065 | `textbox`, `progressbar` (display only) |
| `field-bool` | Yes or no as a field | sensitive F-078 | `switch`, `checkbox` |
| `field-keywords` | Set of labels, chips, comma entry | labels F-063 | `tag` (one pill) |
| `field-reference` (compact, display, cell) | Reference to a person or task; only role input exists | participants F-066, related tasks | `person-picker` (input only), `avatar-group`, `participant` |
| `field-multiline` | Several lines of text | description F-061 | `textbox` (area), `text-view` |
| `field-timezone` | Zone name with search | zone F-069 | none |
| `field-recurrence` | Repeat rule (RRULE) editor and one-line display | repeats F-072 | none |
| `field-duration` | Length of time | estimated duration F-074 | none |
| `field-checklist` | Ticked items inside a task | checklist F-075 | `checkbox` per item |
| `field-link` | Address with the per-use scheme list (ADR 0096 point 11) | links F-076 | `link` |
| `field-color` | Colour pick | colour F-077 | none |

Without these, `choose(field, role)` has no block for the task form (`edit` page of `list.toml`) and the board card facts (due,
priority, participants, keywords). Not an error of the descriptions: the bindings name the fields, the catalog must answer.

## B. Blocks whose slots are not declared (`accepts = []`), so the descriptions place them without bindings

| Component slot missing | Description | Block | Used in |
|---|---|---|---|
| `grid`: `columns` (field, role cell), `rows` (records), `row-menu` | Columns Title, Due, Priority, Status of the list | `grid` | `list.toml` page `list` |
| `task-row`: `rows` (records), `done` (command), `title`, `facts`, `labels`, `group` (grouping) | One task as a line; groups TODAY, READY, BLOCKED | `task-row` | `day.toml` |
| `task-row`: inline open (row opens to show why or when) | Open and close of a row in the day list | `task-row` (nearest `board-detail`, a side frame) | `day.toml` F-011 |
| `quick-add`: `command`, `hint` text | The entry row for the next task (Enter creates) | `quick-add` | `day.toml` |
| `sidebar`: `items` (records of lists, projects, labels), `selected` (the chosen list as source scope) | Projects as headings, labels filterable | `sidebar` | `board.toml`, `day.toml` F-010 |
| `partial-notice`: source and reason | The note above a partial result | `partial-notice` | all |

## C. Board slots missing

| Component slot missing | Description | Block | Nearest |
|---|---|---|---|
| `board`: `card-key` (value, text) | Identifier "FS-n" on the card (F-007) | `board` | `card-facts` (limit 4, already full) |
| `board`: `card-mark` (value or derived, display) | Mark "why it waits" for a blocked card (F-007, F-008 "Hinweis") | `board` | `card-lock` (claims only) |

## D. Filter and host-side

| Component missing | Description | Nearest |
|---|---|---|
| Filter bar (slot kind `condition`) | Inputs bound to a field (label, project, status) | `searchbox`, `tag`, `tablist` |
| Frontend switch | Menu or control that changes the description (old menu Frontend) | none; host or format (README question 2) |

Provider names unproven (no `TypeCatalog`): fields `progress`, `priority`, `due`, `participants`, `keywords`, `note`, `description`,
`timeZone`, `percentComplete`, `recurrenceRules`, `relatedTo`, `sensitive`, `progressExtended`; groupings `board`, `priority`, `standing`;
orders `rank`, `priority`, `due`; commands `create`, `claim`, `release`, `note`, `delete`, `move`, `done`, `reopen`.
