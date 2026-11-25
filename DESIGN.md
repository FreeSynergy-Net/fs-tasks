# fs-tasks – Design

Pattern: Strangler Fig – the data package and the modules take over function by function; the old code stays reachable
under the immutable tag `archive/pre-data-only` until every card has replaced it. Deviation: the old trunk is deleted at
once instead of running in parallel.

fs-tasks is a program without code: a blueprint (`scaffold.toml`) plus forms descriptions (`forms/`) and style data (`styles/`).
The type Task (JSCalendar, RFC 8984; VTODO, RFC 5545) belongs to the module `planning`; frontends only show it
(`covers` in `scaffold.toml`) and send the commands the module serves. The earlier long design (core, server, window) is in the
archive tag; the design of the whole program lives in `fs-documentation/docs/en/programs/apps/tasks/design.md`.
