//! Aufgabenliste: Eigentum (Person oder Gruppe), Recht der fragenden Person, Quelle, Papierkorb-Frist.
//!
//! Pattern: Value Object (Liste, Änderung und Frist als reine Daten; die Rechte rechnet der Rechte-Teil-Trait).

use super::{ObjectId, PersonRef, Revision, Utc};

/// Eine Aufgabenliste (Sammlung, wie ein Kalender; JMAP `TaskList`). Farbe ist die Vorgabe der Liste; Sichtbarkeit und
/// Reihenfolge in der Seitenleiste sind Einstellung der Person.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskList {
    pub id: ObjectId,
    pub name: String,
    pub color: Option<String>,
    /// Eigentümerin: eine Person (`PersonArt::Person`/`Profile`) oder eine Gruppe (`Group`/`Organization`), nie eine
    /// KI oder KI-Gruppe (siehe `mod.rs`, EIGENTUM). Die Aufgaben der Liste gehören ihr mit.
    pub owner: PersonRef,
    /// Was die fragende Person mit dieser Liste darf (Ergebnis, nicht gespeichert).
    pub access: ListAccess,
    pub source: ListSource,
    pub revision: Revision,
    /// `None` = lebt; `Some` = im Papierkorb (nur in `lists` mit `Trash::Include`/`Only`).
    pub trash: Option<Restorable>,
}

/// Was die fragende Person mit einer Liste darf; das Frontend zeigt nur die passenden Knöpfe (nie still ausgegraut:
/// `Forbidden` nennt die Fähigkeit). Quelle ist der gemeinsame Rechte-Teil-Trait (fs-access; Sperre schlägt
/// Erlaubnis). Bis er steht: Eigentümer-Person `Manage`; bei einer Gruppe als Eigentümerin wirksame Mitglieder `Write`,
/// wer die Gruppe verwalten darf (`groups_v1`) `Manage`; eine KI höchstens `Write`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum ListAccess {
    /// Sehen: `list`, `get`, `history`, `changes`.
    Read,
    /// Dazu Aufgaben anlegen, ändern, zuweisen, erledigen, ordnen, Checkliste, übernehmen, löschen.
    Write,
    /// Dazu die Liste umbenennen, umfärben, Eigentum übergeben, löschen, endgültig löschen.
    Manage,
}

/// Woher die Aufgaben einer Liste kommen. PLATZ für Antwort 23: ein `CalDAV`-Konto ist ein eigener Anbieter dieses
/// Traits (gemeinsamer DAV-Wrapper, später); sein Abgleich-Zustand bekommt dann hier eine Variante (nicht brechend,
/// `#[non_exhaustive]`). Heute gibt es nur eigene Listen; Herkunft einzelner Aufgaben steht in `Task::origin`.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ListSource {
    /// Aufgaben entstehen hier (`create`, `TaskExchange::import`).
    Own,
}

/// Änderung einer Liste: `Some` = setzen, `None` = bleibt. Ein leerer Name ist `Invalid`. `owner` übergibt das
/// Eigentum (braucht `ListAccess::Manage` und das Recht, für die neue Eigentümerin anzulegen; KI = `Forbidden`).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct TaskListPatch {
    pub name: Option<String>,
    pub color: Option<Option<String>>,
    pub owner: Option<PersonRef>,
}

/// Bis wann Gelöschtes zurückgeholt werden kann (ADR 0073 Nachtrag 2026-11-16; formgleich mit `appointments_v1`).
/// Die Frist ist eine Einstellung des Programms, nicht des Traits. Die Zusage gilt: nichts geht früher weg. Nach
/// Ablauf geht das Objekt samt Verlauf endgültig (Kaskade). Papierkorb-Ansicht (`Trash::Only`): erst `Until` nach
/// Datum, dann `Held`, dann `Manual`. Vorrang: eine Aufbewahrung (`keep_until`) geht vor Frist UND vor "nie
/// automatisch": solange sie gilt, ist es `Held`; danach `Until` bzw. `Manual`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Restorable {
    /// Frist läuft bis dahin ("noch N Tage").
    Until(Utc),
    /// Aufbewahrt ("aufbewahrt bis …"): `Some(t)` = bis zum SPÄTEREN von Frist und Aufbewahrungsende (so liefert es
    /// fs-objects `restorable_until`); `None` = Aufbewahrung ohne Ende. `purge` ist dann `Forbidden`.
    Held(Option<Utc>),
    /// "Nie automatisch löschen" ist eingestellt: bleibt, bis jemand endgültig löscht (`purge`).
    Manual,
}
