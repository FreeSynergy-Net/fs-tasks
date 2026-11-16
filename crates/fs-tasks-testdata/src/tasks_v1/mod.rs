//! Thema `tasks`, Ziel v1 (Sprung von "tasks 0.1 (draft)", `tasks.capnp` und Trait `TaskItems`). Eigentümer: fs-tasks
//! (ADR 0050). Trait `Tasks`, Grenze `TaskExchange`.
//!
//! Ordner (ADR 0043, jede Datei <= 300 Zeilen), ARBEITS-FIXPUNKT per `tools/hash-ordner.sh tasks_v1` (SHA-256 über
//! sortierte relative Pfade und Inhalte, `LC_ALL=C`). Vorgänger-Fixpunkt `66c7cf13…d085`. Dieser Stand = Delta
//! "Wiederholung ab Erledigung, Checklisten, Gruppe als Zuständige und Eigentümerin, Papierkorb-Frist" (Entscheidung
//! 21, 2026-11-16: vor dem ersten Release IN v1 geändert, kein Sprung auf v2). Feature-Freeze: offen (Nutzer).
//! Dateien: `mod.rs` (dieser Kopf), `shared.rs` (`pub use` aus fs-types, `Error`), `errors.rs` (`TaskError`,
//! `not_possible`), `time.rs` (`Due`, `Span`; `Origin` aus fs-jscalendar), `rule.rs` (Regel aus fs-jscalendar,
//! `RepeatFrom`, `SeriesScope`), `rank.rs` (`Rank`), `list.rs` (Liste, Eigentum, Papierkorb-Frist), `checklist.rs`,
//! `task.rs`, `query.rs`, `api.rs` (Trait `Tasks`), `exchange.rs` (Grenze `TaskExchange`).
//!
//! GEMEINSAME TYPEN (Pins: fs-types = fs-interfaces Tag v0.0.22, fs-jscalendar Tag v0.0.2 mit demselben fs-types;
//! EINE fs-types-Quelle im Lock): Basistypen aus fs-types (`Place` = `PageCursor`), Regel/Erinnerung/Herkunft aus
//! fs-jscalendar (`Recurrence` hält eine Regel außerhalb der Teilmenge als Original-Text, ADR 0030). Formgleich
//! (nicht typgleich) mit `appointments_v1`: `SeriesScope`, `Span`, `Link`, `Restorable`, die Grenz-Typen.
//! `PersonDirectory` (fs-people) und `GroupMembers` (fs-groups) stehen NICHT hier: der Anbieter bekommt sie im
//! Konstruktor (verbraucht).
//!
//! Pattern: Repository (Trait) + Port (ADR 0043/0045). Jede Methode ist Pflicht, keine Standardmethode (Lehre aus
//! `groups_v1`/`appointments_v1`: nichts versteckt Verhalten); was ein Anbieter nicht kann, sagt `capabilities`
//! vorher (kein toter Knopf), und der Aufruf antwortet ausdrücklich `Unsupported` ([`not_possible`]). Formate
//! (VTODO, JSCalendar-Task) stehen NICHT im Fach-Trait, sondern in `TaskExchange`. `#[non_exhaustive]` nur an
//! Ausgabe-Enums (Fehler, Ergebnisse), nie an Structs (der Fake baut sie außerhalb des Crates).
//!
//! Regeln des Vertrags: Wer handelt, ist die fragende Person des Anbieters (`asker`), nie ein Parameter; die Uhr und
//! die Zone der Person (Einstellung) ebenso. Jedes Schreiben auf Bestehendes ist Vergleich-und-Setzen gegen
//! `expected: Revision`; ausgenommen `create`, `create_list`, `restore`, `restore_list`, `purge`, `claim_next`.
//! Ein fallengelassenes Future bricht nichts ab, was schon geschrieben wird (eine Transaktion je Aufruf).
//!
//! Zeiten (ADR 0085): Zeitpunkte `Utc` (ms). Fälligkeit und Beginn sind `Due`: reines Datum (nie umgerechnet), Uhrzeit
//! mit Zone (Pflicht), oder schwebend (`Floating`, nur durch Menschen, die Marke `X-FS-FLOATING` /
//! `freesynergy.net:floating` oder den Kanal `Hub`; eine Zeit ohne Zone ohne Marke ergibt `Timed { zone_assumed:
//! true }`). Menschen (ADR 0085): nur `PersonRef`, nie ein freier Name. Kanban ist eine ANSICHT (ADR 0049): Spalte =
//! `Progress`, Reihenfolge = `rank` (siehe `rank.rs`); keine Board-Objekte. Speicher (ADR 0086): jedes Feld hat eine
//! typisierte Spalte (PLAN.md Abschnitt 3); Checklisten-Punkte sind Zeilen einer Untertabelle, keine Objekte.
//!
//! EIGENTUM (Nutzerantwort 22): Eigentümerin einer Liste ist eine Person ODER eine Gruppe/Organisation
//! ([`TaskList::owner`]); Aufgaben gehören ihrer Liste. Wer was darf, entscheidet der gemeinsame Rechte-Teil-Trait
//! (fs-access, im Entwurf); hier steht nur das Ergebnis für die fragende Person ([`ListAccess`]). KIs und KI-Gruppen
//! sind nie Eigentümer (Eigentum ist Verwaltung, ADR 0056 Punkt 6): `Forbidden { list.create | list.edit }`.
//!
//! ZUSTÄNDIGE UND ÜBERNAHME (`assign`, `claim`): Zuständig sein kann eine Person oder eine Gruppe (`PersonRef` mit
//! `PersonArt::Group`/`Organization`; aufgelöst über `GroupMembers` von fs-groups, zur Zeit der Frage, nie
//! gespeichert). Eine Gruppe antwortet nicht (`respond` gilt nur der eigenen Zeile einer Person); ein Mitglied
//! übernimmt mit `claim`. claim-Regel: übernehmen darf, wer (a) selbst zuständig ist, (b) wirksames Mitglied einer
//! zuständigen Gruppe ist, oder (c) bei einer Aufgabe ohne Zuständige Schreibrecht an der Liste hat; sonst
//! `Forbidden { task.claim }`. (b) prüft der Anbieter über `GroupMembers::memberships_of(asker)` geschnitten mit den
//! zuständigen Gruppen, NIE über `members_of(gruppe)`: so gibt eine sensible Gruppe einer KI kein `NotFound` preis, die
//! Antwort ist immer dasselbe `Forbidden { task.claim }`. Es hält immer höchstens EINE Person (mehrere Menschen:
//! einzeln zuweisen oder Teilaufgaben); die Zuständigen ändern sich durch `claim` nicht. Die Übernahme endet mit
//! `release`, mit `until`, mit `Completed`/`Cancelled`/`Failed` und beim Weiterrücken einer Serie. KI-Regeln (ADR 0054,
//! Antwort 29): eine KI ist nie Mitglied einer Menschengruppe, übernimmt also nur, was ihr selbst, ihrer KI-Gruppe oder
//! niemandem in einer Liste mit ihrem Schreibrecht gehört; "STOP KI" (Rechte entzogen) beendet alle ihre Übernahmen
//! sofort (`Leave`).
//!
//! PAPIERKORB (ADR 0073 Nachtrag 2026-11-16): Löschen = Papierkorb mit Frist ([`Restorable`]); die Frist ist eine
//! Pflicht-Einstellung des Programms (Standard 30 Tage, mindestens 1 Tag, "nie automatisch" erlaubt), keine
//! Trait-Sache; eine Aufbewahrung (`keep_until`) geht vor; nach Ablauf oder `purge` geht das Objekt samt Verlauf und
//! Checkliste (Kaskade). Eine Liste nimmt dabei auch ihre vorher einzeln gelöschten Aufgaben mit; eine aufbewahrte
//! darin hält die Liste im Papierkorb (`purge` = `Forbidden`, wie `appointments_v1`, siehe `Tasks::purge`).
//!
//! TEILAUFGABEN SIND EIN BAUM (PLAN O18): eine Aufgabe hat höchstens EINE Eltern-Aufgabe (`parent`), nie zwei; ein Kind
//! unter einem lebenden zweiten Elternteil gibt es darum nicht. Höchstens [`SUBTASK_DEPTH_MAX`] Ebenen (die oberste
//! Aufgabe ist Ebene 1): `create`, `update` (`parent`, `list`) und `restore`, die einen tieferen Baum ergäben, sind
//! `Invalid` (`parent`); geprüft wird der ganze mitgenommene Teilbaum. `delete` nimmt genau die lebenden Teilaufgaben
//! mit (`Deleted::with_subtasks`); nur sie gehen beim endgültigen Löschen des Elternteils mit. Eine schon vorher
//! einzeln gelöschte oder aufbewahrte Teilaufgabe behält ihre eigene Frist: geht der Elternteil endgültig, wird ihr
//! `parent` gelöst (oberste Aufgabe), sie wird nicht mitgelöscht (geht ihre LISTE endgültig, geht sie mit; Anbieter
//! über fs-objects: vor `delete_final` in derselben Transaktion abhängen). `restore` einer Teilaufgabe, deren
//! Elternteil im Papierkorb liegt, ist `Invalid` (`parent`): zuerst den Elternteil.
//!
//! ERWEITERUNGEN (Namensliste für den Nachtrag zu ADR 0085; nichts doppelt erfinden): JSCalendar-Eigenschaften
//! `freesynergy.net:floating`, `freesynergy.net:checklist`, `freesynergy.net:repeatFrom`; VTODO `X-FS-FLOATING`,
//! `X-FS-CHECKLIST`, `X-FS-REPEAT-FROM` (Wert `COMPLETION`). Ein Ziel ohne sie bekommt den Verlust in `lost`.

mod api;
mod checklist;
mod errors;
mod exchange;
mod list;
mod query;
mod rank;
mod rule;
pub mod shared;
mod task;
mod time;

pub use api::*;
pub use checklist::*;
pub use errors::*;
pub use exchange::*;
pub use list::*;
pub use query::*;
pub use rank::*;
pub use rule::*;
pub use shared::*;
pub use task::*;
pub use time::*;
