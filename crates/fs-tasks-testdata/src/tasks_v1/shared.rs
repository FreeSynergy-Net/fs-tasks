//! Gemeinsame Typen: Wiederverwendung von `fs-types` (>= 0.0.22), kein Stand-in mehr. `Place` (Weiterblätter-Stelle)
//! heißt dort `PageCursor` (`Place` ist dort der `geo:`-Punkt); hier unter dem alten Namen. `Error`
//! bleibt hier: der Fehler des Themas, kein gemeinsamer Basistyp. Ein eigenes `Result`-Alias gibt es bewusst nicht
//! (es würde über `pub use` das `Result` des Prelude überschatten); Ergebnisse heißen `TaskResult`.
//!
//! Pattern: Value Object (Basistypen aus fs-types; Eintritt nur über `parse`/Konstruktoren).

pub use fs_types::PageCursor as Place;
pub use fs_types::{
    BasicValue, Capabilities, Date, FileRef, LocalDateTime, MediaType, ObjectId, Page, PersonArt,
    PersonRef, Problem, Reference, Revision, TypeProblem, Utc, Zone, ZoneKind,
};

/// Fehler des Themas. Eine unbekannte oder hier nicht gültige Zone ist KEIN eigener Fehler (wie `appointments_v1`,
/// Entscheidung 2a): sie ist `Invalid` mit einem [`Problem`] am Feld (`due.zone`, `start.zone`, `view_zone`,
/// `assume_zone` …) und dem Code der Zonenprüfung aus [`TypeProblem`] (heute `zone.bad`); nie raten (ADR 0085).
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Error {
    /// Der Anbieter kann es ausdrücklich nicht (ADR 0045), mit Fähigkeitskennung.
    Unsupported {
        capability: String,
        reason: String,
    },
    /// Vergleich-und-Setzen gescheitert: jemand hat vorher geschrieben.
    Changed {
        current: Revision,
    },
    NotFound(ObjectId),
    /// Nicht erlaubt (z. B. eine fremde Liste, die man nur lesen darf), mit Fähigkeitskennung. Nicht
    /// `Unsupported`: der Anbieter kann es, die fragende Person darf es nicht.
    Forbidden {
        capability: String,
    },
    /// Prüfung gescheitert; nichts geschrieben. Je Feld ein [`Problem`] mit Code (auch die Zone, siehe oben).
    Invalid(Vec<Problem>),
    /// Speicher oder Anbieter nicht erreichbar.
    Unavailable(String),
}
