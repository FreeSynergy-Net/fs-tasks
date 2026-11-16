//! Thema `tasks`, Ziel v1 (Sprung von "tasks 0.1 (draft)", `tasks.capnp` und Trait `TaskItems`). Eigentümer: fs-tasks
//! (ADR 0050).
//!
//! FESTER Trait des Piloten `programm-grundgeruest` (Stufe 2). Nach `strategie-ok` wird dieser ORDNER per SHA-256
//! gesperrt (`tools/hash-ordner.sh tasks_v1`: sortierte relative Pfade und Inhalte, `LC_ALL=C`); jede Änderung ist ein
//! Sprung (ADR 0045) und ein neuer Plan. Stufe 3 kopiert den Ordner nach `fs-tasks-core/src/interface/tasks_v1/`.
//!
//! GEMEINSAME TYPEN: `shared.rs` ist `pub use` auf `fs-types` (ab v0.0.22, fs-interfaces); `Place` (Weiterblätter-Stelle)
//! heißt dort `PageCursor`. Eine Stand-in-Kopie gibt es nicht mehr, und die Datei ist NICHT mehr byte-gleich mit
//! `appointments_v1/shared.rs` (gesperrter Kalender-Ordner, Hash 2b7fc98b…19b8, nicht angefasst): hier stehen zusätzlich
//! der `Error` des Themas. Wiederholungsregel (`rule.rs`, `Recurrence` hält bei Regeln außerhalb der Teilmenge den
//! Original-Text), `Origin` und `Alert` kommen per `pub use` aus fs-jscalendar (Entscheidung 2026-11-16; die Sperre
//! nennt den Tag von fs-jscalendar mit, siehe Opus-Review J0); `RelativeTo::End`
//! heißt bei Aufgaben `due`. Umwandlungstest gegen den gesperrten Kalender: `pilot-calendar/fs-jscalendar-umwandlung-test.rs`.
//! Formgleich (nicht typgleich) mit dem Kalender bleiben `SeriesScope`, `Span`, `Link` und die Grenz-Typen in `exchange.rs`;
//! sie ziehen in v2 beider Traits nach fs-jscalendar bzw. fs-interfaces.
//! `PersonDirectory` steht NICHT hier: Personen sucht das Frontend über den Trait von fs-people (verbraucht).
//!
//! Pattern: Repository (interface) + Null Object (ADR 0043/0045) - jeder Anbieter hat jede Methode; was er nicht kann,
//! antwortet `Unsupported` (Standardmethode; Pflichtmethoden ohne Standard sind `capabilities`, `asker`, `lists`, `get`,
//! `get_by_uid`, `list`, `changes`), und `capabilities` sagt es vorher, damit kein Frontend einen toten Knopf zeigt. Formate (VTODO, JSCalendar-Task) stehen NICHT im Fach-Trait, sondern in `TaskExchange` (Grenze).
//!
//! Zeiten (ADR 0085): Zeitpunkte `Utc` (ms). Fälligkeit und Beginn sind `Due`: reines Datum (nie umgerechnet, nie
//! "17:00"), Uhrzeit mit Zone (Pflicht), oder schwebend (`Floating`, nur durch Menschen, die eigene Marke
//! `X-FS-FLOATING` / `freesynergy.net:floating` oder den Kanal `Hub`; eine Zeit ohne Zone ohne Marke ergibt
//! `Timed { zone_assumed: true }`). Menschen (ADR 0085): nur `PersonRef`, nie ein freier Name. Wer handelt, ist die
//! fragende Person des Anbieters (`asker`), kein Parameter. Kanban ist eine ANSICHT (ADR 0049): Spalte = `Progress`,
//! Reihenfolge = `rank` (Bruchindex `Rank`: `place` schreibt nur die bewegte Aufgabe; nur bei `Exhausted`/`Equal` verteilt
//! der Anbieter die Rangfolge einmal neu, siehe `rank.rs`); es gibt keine Board-Objekte und keinen Board-Speicher.
//! Speicher (ADR 0086): jedes Feld hat eine typisierte Spalte (PLAN.md Abschnitt 3).

mod api;
mod errors;
mod exchange;
mod query;
mod rank;
mod rule;
pub mod shared;
mod task;
mod time;

pub use api::*;
pub use errors::*;
pub use exchange::*;
pub use query::*;
pub use rank::*;
pub use rule::*;
pub use shared::*;
pub use task::*;
pub use time::*;
