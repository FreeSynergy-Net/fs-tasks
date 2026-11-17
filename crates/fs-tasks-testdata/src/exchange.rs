//! `FakeExchange`: the `TaskExchange` of the fake, honest about what it cannot do.
//!
//! Pattern: Null Object (the fake keeps no files, so it answers `formats()` with `No` in both directions and says
//! `Unsupported` ([`not_possible`]) for import and export, as the trait has no default methods; the format adapters
//! are step K3).

use crate::tasks_v1::{
    ExportReport, ExportTarget, FileRef, FormatSupport, ImportContext, ImportReport, Level,
    MediaType, ObjectId, TaskExchange, TaskResult, not_possible,
};
use async_trait::async_trait;

/// The two formats of the boundary: VTODO (`text/calendar`) and the JSCalendar task.
pub const MEDIA_TYPES: [&str; 2] = ["text/calendar", "application/jscalendar+json"];

/// An exchange that offers nothing (yet).
#[derive(Clone, Copy, Debug, Default)]
pub struct FakeExchange;

#[async_trait]
impl TaskExchange for FakeExchange {
    fn formats(&self) -> Vec<FormatSupport> {
        MEDIA_TYPES
            .iter()
            .map(|m| FormatSupport {
                media_type: MediaType((*m).to_owned()),
                import: Level::No,
                export: Level::No,
            })
            .collect()
    }

    async fn import(
        &self,
        _source: FileRef,
        _as_type: &MediaType,
        _ctx: &ImportContext,
    ) -> TaskResult<ImportReport> {
        not_possible("exchange.import")
    }

    async fn export(&self, _ids: &[ObjectId], _to: &ExportTarget) -> TaskResult<ExportReport> {
        not_possible("exchange.export")
    }
}
