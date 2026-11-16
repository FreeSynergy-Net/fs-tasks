//! Wiederholung: Typen aus fs-jscalendar; hier nur die Serienfrage der Aufgabe.
//!
//! Pattern: Value Object (Regel als reine Daten, der Typ kommt geteilt aus fs-jscalendar; die Engine ist geteilt).

use super::LocalDateTime;

// Wiederholungsregel (RFC 8984 `RecurrenceRule`), Wochentag, Häufigkeit, Überspringen, `NDay`, `ByMonth` und `RuleEnd`
// kommen aus fs-jscalendar (Entscheidung 2026-11-16: kein Stand-in, keine Kopie). Bezugspunkt der Regel ist `start`,
// sonst `due` (RFC 8984 5.2: eine wiederholte Aufgabe hat eins von beiden, sonst `Invalid`). Bei `RuleEnd::Until` einer
// Aufgabe mit reinem Datum ist der Tag mit `T00:00:00` gemeint. Eine Regel außerhalb der Teilmenge, die der Anbieter
// rechnen kann, bleibt als `Recurrence::Original` (Text) erhalten, steht im Loss-Bericht und ist beim Bearbeiten
// `Unsupported` (Entscheidung Kalender Schritt 3, Punkt 1).
pub use fs_jscalendar::recurrence::{
    ByMonth, Frequency, NDay, Recurrence, RecurrenceRule, RuleEnd, Skip, Weekday,
};

/// Umfang einer Änderung an einer wiederholten Aufgabe (Serienfrage, wie Kalender A9/A10). Für Einzelaufgaben nur
/// `Whole`. Das Vorkommen heißt nach seinem ursprünglichen lokalen Bezugspunkt (`start`, sonst `due`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeriesScope {
    /// Die ganze Aufgabe bzw. die ganze Serie.
    Whole,
    /// Nur dieses Vorkommen (wird eine Ausnahme).
    This { recurrence_id: LocalDateTime },
    /// Dieses und alle folgenden: die Serie wird geteilt (neue `uid`, `Relation::Next`/`First`).
    ThisAndFollowing { recurrence_id: LocalDateTime },
}
