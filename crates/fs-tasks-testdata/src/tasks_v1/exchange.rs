//! Grenze (ADR 0085 Abschnitt 1): VTODO (RFC 5545) und JSCalendar-Task (RFC 8984) als Anbieter außerhalb des Fach-Traits.
//!
//! Pattern: Adapter (Formate an der Grenze, hinter einem Trait). Die Typen sind formgleich mit `appointments_v1/exchange.rs`
//! (v2 beider Traits: nach fs-interfaces). Personen werden über den `PersonDirectory` von fs-people normalisiert, den der
//! Anbieter im Konstruktor bekommt (nicht Teil dieser Signaturen).

use super::{not_possible, FileRef, MediaType, ObjectId, Problem, Reference, TaskResult, Zone};
use async_trait::async_trait;

/// Fähigkeit eines Formats je Richtung.
#[derive(Clone, Debug, PartialEq, Eq)]
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
#[async_trait]
pub trait TaskExchange: Send + Sync {
    fn formats(&self) -> Vec<FormatSupport>;
    /// `exchange.import`: unbekannte Felder bleiben im Original und stehen in `lost` (ADR 0086 Punkt 5).
    async fn import(
        &self,
        _source: FileRef,
        _as_type: &MediaType,
        _ctx: &ImportContext,
    ) -> TaskResult<ImportReport> {
        not_possible("exchange.import")
    }
    /// `exchange.export`: Ziel = Quelle und unverändert = Original Byte für Byte; eine unbekannte Id ergibt `NotFound`
    /// für den ganzen Aufruf.
    async fn export(&self, _ids: &[ObjectId], _to: &ExportTarget) -> TaskResult<ExportReport> {
        not_possible("exchange.export")
    }
}
