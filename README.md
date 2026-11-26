# FreeSynergy Tasks – data package

fs-tasks is a **data package**: it holds no program code. Tasks, dependencies and boards are served by the module
`planning` (repo `fs-modules-planning`, owner of the JSCalendar `Task`, RFC 8984); the program is the description of
how to show them.

- `scaffold.toml` – the blueprint: program `tasks`, the module `planning` (0.0.8), the display components.
- `forms/` – the frontends as forms descriptions (board, list, day and board variants).
- `styles/` – style data of the frontends.
- `docs/worker-protocol.md` – the old worker protocol (kept until card M6 moves it to fs-documentation).

The package is one noarch RPM `fs-tasks` (`fs-blueprint package rpm`, built by CI on a tag). It requires the host
(`fs-starter`), the module `fs-module-planning` and `freesynergy-filesystem`.

## What moved

Version 0.0.11 removed the old Rust code (core, server, command line, window, tests). Nothing is lost: the state before the
conversion is the annotated tag `archive/pre-data-only` (commit `2f24768f967a5c42541aba8b9fd233772e2d3e1c`), and every
piece that is not yet in a module has a card (M1 to M14) in the FreeSynergy checklist. Until those cards are done, the
command line (`fs-tasks add/list/claim/next`), iCalendar import/export in the window and the status command check are not
available from this package.

## Licence and rules

Dual licensed `MIT OR Apache-2.0` (`LICENSE-MIT`, `LICENSE-APACHE`). `.fs-rules.toml` sets the coding rules the CI gate reads (`blinc = "components-only"`: Blinc is used only through `fs-components`; this package has no code, the rule keeps it so).

Written with the help of Claude (Anthropic).
