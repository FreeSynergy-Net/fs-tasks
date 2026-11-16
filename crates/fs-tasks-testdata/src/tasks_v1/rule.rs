//! Wiederholung: Typen aus fs-jscalendar; hier die Zähl-Basis (`RepeatFrom`) und die Serienfrage der Aufgabe.
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

/// Wovon die nächste Fälligkeit einer wiederholten Aufgabe zählt (Nutzerantwort 28; Vikunja, Taskwarrior, Things
/// "nach Erledigung"). RFC 8984 kennt nur den Plan; `Completion` ist die Erweiterung `freesynergy.net:repeatFrom`
/// (VTODO `X-FS-REPEAT-FROM:COMPLETION`). Die Regel selbst bleibt unverändert gespeichert (ADR 0030); ein Ziel ohne
/// die Erweiterung bekommt die Regel als Plan und den Verlust in `lost`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum RepeatFrom {
    /// Nach Plan (`from_due`, RFC 8984): die Vorkommen stehen vorher fest; Erledigen eines Vorkommens (`This`) macht
    /// das nächste nicht erledigte zum aktuellen, auch wenn es schon überfällig ist. Serien-Pfad `This`,
    /// `ThisAndFollowing` (Teilen) wie im Kalender.
    #[default]
    Schedule,
    /// Ab Erledigung (`from_completion`): erst das Erledigen legt das nächste Vorkommen fest. Regeln:
    /// - FORM: genau eine `Recurrence::Rule` mit `frequency`, `interval`, `end` (sonst Vorgaben, keine `by_*`-Teile),
    ///   keine `excluded_recurrence`; anderes ist `Invalid` (`recurrence`). Stunden-/Minutenregeln nur bei `Timed`.
    /// - ZÄHLEN: nächster Bezugspunkt = der lokale Tag des Erledigens + `interval` Schritte der `frequency` (Monat und
    ///   Jahr: derselbe Tag, am Monatsende der letzte); die Uhrzeit des Bezugspunkts bleibt. Tag des Erledigens:
    ///   bei `Timed` in dessen Zone, bei `Date`/`Floating` in der Zone der fragenden Person (Einstellung, sonst Gerät;
    ///   wie `ZoneSource`). Eine lokale Zeit, die es dort nicht gibt (Umstellung), rückt vor wie in der Engine. Hat
    ///   die Aufgabe `start` UND `due`, rücken beide um dasselbe; der Abstand bleibt.
    /// - ÜBERFÄLLIG oder VERFRÜHT: es zählt nur der Tag des Erledigens, nie die alte Fälligkeit (kein Stau
    ///   überfälliger Vorkommen; wer früher erledigt, ist früher wieder dran).
    /// - ENDE: `Count(n)` zählt die Erledigungen (der Anbieter schreibt beim Weiterrücken `Count(n - 1)`; die Regel
    ///   beschreibt immer den Rest ab dem aktuellen Bezugspunkt); `Count(1)` erledigt = Serie fertig. `Until`: läge der
    ///   nächste Bezugspunkt danach, ist die Serie fertig.
    /// - WEITERRÜCKEN = Fortsetzung DERSELBEN Aufgabe (gleiche `id`, gleiche `uid`, `sequence + 1`): `progress
    ///   (Completed)` am aktuellen Vorkommen (`Whole` oder `This`) legt eine Ausnahme `Changed { progress: Completed }`
    ///   unter dem alten Bezugspunkt an (Logbuch, Export als erledigtes Vorkommen), verschiebt `start`/`due`, setzt
    ///   `progress` zurück auf `NeedsAction`, entfernt die Haken der Checkliste und beendet die Übernahme; ein
    ///   Aufruf, eine Transaktion, ein Verlaufseintrag `Update`. Gewählt statt neuer Aufgabe je Vorkommen: die `uid`
    ///   bleibt für `CalDAV`-Abgleich und Verweise (`dependencies`, Links) stabil, Zuständige und Checkliste
    ///   werden nicht kopiert.
    /// - RÜCKGÄNGIG: `progress(NeedsAction, This { alter Bezugspunkt })` an der JÜNGSTEN solchen Ausnahme setzt den
    ///   Bezugspunkt zurück (`Count` + 1) und entfernt die Ausnahme; an älteren ist es `Invalid`.
    /// - SERIENFRAGE: `This` gilt nur dem aktuellen Vorkommen (`delete(This)` = auslassen: rückt weiter wie Erledigen,
    ///   Ausnahme `Excluded`); `ThisAndFollowing` am aktuellen Vorkommen ist `Whole`; künftige Vorkommen gibt es noch
    ///   nicht (`Invalid`). Wechsel zwischen `Schedule` und `Completion` nur mit `Whole` (`TaskPatch::repeat_from`).
    Completion,
}

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
