//! Checkliste einer Aufgabe: leichte Punkte zum Abhaken (Nutzerantwort 28), keine Teilaufgaben.
//!
//! Pattern: Value Object (Punkt und Änderung als reine Daten; eine Änderung = eine Transaktion an der Aufgabe).

use super::{PersonRef, Rank, Utc};

/// Ein Punkt der Checkliste (Things "Checkliste", MS To Do "Schritte").
///
/// ABGRENZUNG zur Teilaufgabe (`parent`): ein Punkt ist nur Text, Haken und Platz. Er hat keine Fälligkeit, keine
/// Zuständigen, keine Labels, keine Abhängigkeiten, keine Übernahme, keine eigene Revision, keinen eigenen Papierkorb
/// und erscheint nicht als Zeile in `list` (die Suche `text` findet die Aufgabe auch über ihre Punkte). Wer mehr
/// braucht, legt eine Teilaufgabe an. Die Punkte gehören der Aufgabe: sie gehen mit ihr in den Papierkorb, kommen mit
/// ihr zurück und gehen mit ihr endgültig; jede Änderung erhöht die Revision der Aufgabe und steht in ihrem Verlauf
/// (`row_key` = `key`). Bei einer Serie gilt die Checkliste der ganzen Serie; die Haken gehören zum aktuellen
/// Vorkommen und werden beim Weiterrücken (erledigt, ausgelassen) alle entfernt (ein `Update` im Verlauf).
/// Austausch: JSCalendar-Erweiterung `freesynergy.net:checklist`, VTODO `X-FS-CHECKLIST`; wo das Ziel es nicht kennt,
/// steht es in `lost` (nie still verloren).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChecklistItem {
    /// Schlüssel innerhalb der Aufgabe (vom Anbieter vergeben, bleibt beim Umbenennen und Verschieben).
    pub key: String,
    pub title: String,
    /// `None` = offen; `Some` = abgehakt, von wem und wann.
    pub done: Option<Checked>,
    /// Hand-Reihenfolge innerhalb DIESER Aufgabe (eigene Rangfolge, Regeln wie [`Rank`]; das Neu-Verteilen betrifft nur
    /// die Punkte dieser Aufgabe und bleibt in derselben Transaktion).
    pub rank: Rank,
}

/// Wer einen Punkt wann abgehakt hat (für Gruppenlisten: "Anna hat X erledigt").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checked {
    pub by: PersonRef,
    pub at: Utc,
}

/// Eine Änderung der Checkliste (`Tasks::checklist`). `after: None` = ganz oben; ein unbekannter `key` ist `Invalid`
/// (`checklist.key`), ein leerer Titel ebenso, ein `Add` über [`CHECKLIST_MAX`] auch. Verlauf: `Add`/`Remove` (Feld
/// `checklist`), `Update` (`checklist.title`, `checklist.done`), `Move` (`checklist.rank`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChecklistChange {
    Add {
        title: String,
        after: Option<String>,
    },
    Rename {
        key: String,
        title: String,
    },
    /// Abhaken (`true`) oder wieder öffnen (`false`); schon so = keine Änderung, keine neue Revision.
    Check {
        key: String,
        done: bool,
    },
    Place {
        key: String,
        after: Option<String>,
    },
    Remove {
        key: String,
    },
}

/// Höchstzahl der Punkte einer Checkliste; `Add` darüber und ein `TaskDraft::checklist` mit mehr Punkten sind
/// `Invalid` (`checklist`); beim Import stehen überzählige Punkte in `lost`. Wer mehr braucht, legt Teilaufgaben an.
pub const CHECKLIST_MAX: usize = 100;
