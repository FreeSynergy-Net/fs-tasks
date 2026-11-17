//! Fälligkeit, Dauer und Herkunft einer Aufgabe (Herkunft aus fs-jscalendar).
//!
//! Pattern: Value Object (Zeit und Herkunft als reine Daten).

use super::{Date, LocalDateTime, Zone};

/// Wann eine Aufgabe fällig ist bzw. beginnt (ADR 0085 "fs-tasks": `Due { Date | Timed | Floating }`). Genau eine Form.
/// Eine Fälligkeit ist ein Stichtag, kein Zeitraum: sie hat keine Länge (die geschätzte Dauer ist `estimated`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Due {
    /// Reines Datum ohne Uhrzeit: keine Zone, nie umgerechnet, nie als Uhrzeit gezeigt. Export: `JSCalendar`
    /// `T00:00:00` + `showWithoutTime`, ohne `timeZone`; iCalendar `VALUE=DATE`.
    Date(Date),
    /// Mit Uhrzeit: lokale Zeit in `zone` (Pflicht). `zone_assumed`: die Zone wurde an der Grenze für eine Zeit ohne
    /// Zone eingesetzt (Import, Migration; ADR 0085) und wird sichtbar markiert.
    Timed {
        at: LocalDateTime,
        zone: Zone,
        zone_assumed: bool,
    },
    /// Überall dieselbe Wanduhrzeit (RFC 8984 1.4.5): kein Zeitpunkt, nie umgerechnet, in der Zone des Betrachters
    /// gezeigt. Entsteht nur durch Menschen, die eigene Marke oder den Kanal `Hub` (siehe `mod.rs`).
    Floating(LocalDateTime),
}

/// Dauer nach `JSCalendar` `Duration` (`estimatedDuration`, Abstand einer Abhängigkeit): Tage nominal, Sekunden genau.
/// Formgleich mit `appointments_v1::Span`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Span {
    pub days: u32,
    pub seconds: u32,
}

// Herkunft eines importierten Objekts (fs-objects `object_origin`, nur lesend) und ihr Kanal kommen aus fs-jscalendar
// (Entscheidung 2026-11-16: nicht kopieren). "Unverändert seit Import" wird berechnet: `revision == rev_at_import`.
// `Google`/`Outlook` stehen für Google Tasks und Microsoft To Do (geschlossen: nur Import). JMAP gibt es bewusst nicht in v1.
pub use fs_jscalendar::origin::{Origin, OriginChannel};
