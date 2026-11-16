//! `FakeExchange`: the `TaskExchange` of the fake, honest about what it cannot do.
//!
//! Pattern: Null Object (the fake keeps no files, so it answers `formats()` with `No` in both directions and the
//! trait defaults say `Unsupported` for import and export; the format adapters are step K3).

use crate::tasks_v1::{FormatSupport, Level, MediaType, TaskExchange};
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
}
