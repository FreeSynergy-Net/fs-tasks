//! Grenze (ADR 0085 Abschnitt 1): VTODO (RFC 5545) und JSCalendar-Task (RFC 8984) als Anbieter außerhalb des Fach-Traits.
//!
//! Pattern: Adapter (Formate an der Grenze, hinter einem Trait). Die Typen sind formgleich mit `appointments_v1/exchange.rs`
//! (v2 beider Traits: nach fs-interfaces). Personen werden über den `PersonDirectory` von fs-people normalisiert, den der
//! Anbieter im Konstruktor bekommt (nicht Teil dieser Signaturen).

use super::{FileRef, MediaType, ObjectId, Problem, Reference, TaskResult, Zone};
use async_trait::async_trait;

/// Fähigkeit eines Formats je Richtung.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Level {
    Full,
    Partial { missing: Vec<String> },
    No,
}
/// Was ein Format-Anbieter kann.
#[derive(Clone, Debug)]
pub struct FormatSupport {
    pub media_type: MediaType,
    pub import: Level,
    pub export: Level,
}
/// Was beim Umwandeln nicht abbildbar war (nie still verloren; das Original behält es).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Loss {
    pub uid: Option<String>,
    pub field: String,
    pub reason: String,
}
/// Woher die eingesetzte Zone stammt (wird gemeldet).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZoneSource {
    PersonSetting,
    Device,
    LastKnownDevice,
    UtcFallback,
}
/// Grundlage der Normalisierung beim Import: Zielliste und Zone für Zeiten ohne Zone (nie still `UTC`).
#[derive(Clone, Debug)]
pub struct ImportContext {
    pub list: ObjectId,
    pub assume_zone: Zone,
    pub assume_zone_source: ZoneSource,
}
/// Ergebnis eines Imports.
#[derive(Clone, Debug, Default)]
pub struct ImportReport {
    pub created: Vec<ObjectId>,
    /// Schon bekannte `uid`s (nicht doppelt angelegt).
    pub skipped: Vec<String>,
    pub failed: Vec<Problem>,
    pub lost: Vec<Loss>,
    /// Zeiten, deren Zone eingesetzt wurde (`zone_assumed`), je `uid`.
    pub zone_assumed: Vec<String>,
    /// Sammelvorschlag Personen (ADR 0058/0085), Id des Vorschlags.
    pub people_proposal: Option<ObjectId>,
    /// Termine (`VEVENT`) aus der Datei, weitergegeben an fs-calendar (ADR 0050), je `uid`.
    pub forwarded_events: Vec<String>,
}
/// Ziel eines Exports: Medientyp und, bei Konten, die Quell-IRI (Ziel = Quelle entscheidet die Rundlauf-Regel).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExportTarget {
    pub media_type: MediaType,
    pub destination: Option<Reference>,
}
/// Ergebnis eines Exports.
#[derive(Clone, Debug)]
pub struct ExportReport {
    pub written: FileRef,
    pub lost: Vec<Loss>,
}

/// Grenz-Adapter eines Formats (`text/calendar` mit `VTODO`, `application/jscalendar+json` mit `Task`). Liest, normalisiert
/// (Zeiten, Personen) und schreibt über [`super::Tasks`]; Export ist ein eigener Befehl, nie Speichern (ADR 0046).
/// Jede Methode ist Pflicht; ein Format ohne Import oder Export meldet es in `formats` (`Level::No`) und antwortet
/// mit [`super::not_possible`]. Checkliste und `RepeatFrom::Completion` reisen als Erweiterung
/// (`freesynergy.net:checklist`, `freesynergy.net:repeatFrom`; VTODO `X-FS-…`), sonst stehen sie in `lost`.
///
/// AB ERLEDIGUNG im Export: die erledigten (ausgelassenen) Vorkommen liegen unter alten Bezugspunkten, die nicht auf
/// dem Raster der heutigen Regel liegen. JSCalendar-Task: als Schlüssel in `recurrenceOverrides` gültig (RFC 8984
/// 4.3.5, Zusatz-Vorkommen). VTODO: je solches Vorkommen ein `RDATE` plus die Ausnahme mit `RECURRENCE-ID` (Stand
/// `COMPLETED` usw.); kann das Ziel das nicht, stehen sie in `lost` (nie still weg). Verdichtete (nur noch im Verlauf,
/// [`super::REPEAT_DONE_KEPT`]) gehören nicht mehr zur Aufgabe und werden nicht exportiert.
#[async_trait]
pub trait TaskExchange: Send + Sync {
    fn formats(&self) -> Vec<FormatSupport>;
    /// `exchange.import`: unbekannte Felder bleiben im Original und stehen in `lost` (ADR 0086 Punkt 5).
    async fn import(
        &self,
        source: FileRef,
        as_type: &MediaType,
        ctx: &ImportContext,
    ) -> TaskResult<ImportReport>;
    /// `exchange.export`: Ziel = Quelle und unverändert = Original Byte für Byte; eine unbekannte Id ergibt `NotFound`
    /// für den ganzen Aufruf.
    async fn export(&self, ids: &[ObjectId], to: &ExportTarget) -> TaskResult<ExportReport>;
}
