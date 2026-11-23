# Placeholders: parts of the old fs-tasks that the three descriptions do not carry yet

Nothing is dropped: each line keeps the id of LISTE-fs-tasks-funktionen-vorher-2026-11-18 (main 89cb7f4e) and says why it is a
placeholder. "Host" = the forms host (`fs-forms-run`), not the description. Marked PLATZHALTER = to be filled, then removed here.

| Id | Old part | Why not in a description | Next step |
|---|---|---|---|
| F-004 | Compact classic window (one view, no menu) | A size or state of the host, not a layout | host question: compact rule from window size |
| F-012 | Start list `--list today` | Start option, not content | settings `fs.shell.*` or host option (question 4) |
| F-013 | Trash (restore, 30 days) | Not in the old window (storage layer only, UNPROVEN) | own view when the provider exposes `restore`; nearest block `sortable-list` |
| F-014 | Filters project, status, label | Filter tree not read by the format cut; slot kind `condition` has no catalog block | extend format and add a filter bar component (FEHLENDE-COMPONENTS.md) |
| F-015 | Search | No search field in the old window (UNPROVEN); block `searchbox` exists, no slot | decide with the user; then `searchbox` + view `search` |
| F-016 | Calendar or time view | Not an old view; only iCalendar import and export | would be a view of the calendars program, not tasks |
| F-017 | Problem window (server does not answer, Yes / No) | Host error with codes (`host-error-*`), blocks `confirm-dialog` or `choice-dialog` | host; no description content |
| F-018 | Notices ("someone else changed this task", "enter a title") | Rule texts of the program and `partial-notice`, host-shown | host; each text is a program code with an MF2 key |
| F-019 | Picture of a view (`--snapshot`) | Host function (draw to PNG) | host; picture pairs per frontend for acceptance |
| M-004..M-006 | Menu Frontend > Classic, Board, Day list | The format has no menu kind for "switch description" | question 2 in README.md |
| M-007 | Problem window menu Yes / No / Close | Host dialog | host |
| F-090..F-096 | Help, appearance, self-update | Help = CLI `--help`; appearance size only; self-update does not exist (V1.1) | not description content |
| F-100..F-103 | Settings, language, accessibility | Settings live in the settings database (ADR 0100); accessibility UNPROVEN | settings declaration of the blueprint |
| Rules F-040..F-051 | Rank, blocked, WIP, trash, claim, time, limits, secrets, series | Rules are never in a description (ADR 0096 point 1); they belong to the provider | `[[ruleset]]` of the blueprint |
| Fields F-066..F-083 | Cost, start, alerts, estimated duration, checklist, links, colour, origin, check, run, actor | Not bound in any view of the old window (UNPROVEN or not shown) | add rows to `edit` once field blocks exist; `links` needs a scheme list per use |
| F-028, F-029, F-032 | Board commands `claim`, `release`, `note` (menu, card menu, panel actions) | The module `planning` has no such commands yet (no service behind them, so they are not in its `catalog/commands.json`; the contract `tasks.capnp` has no call for them, and no `note` member). A button must not point to a command that does not exist; they were taken out of `board.toml` (Comes later, they come back with the commands) | commands in `planning` after a contract step (claim, release, note) |
