//! Time helpers: the typed values of the trait on one side, `chrono` and the tz database on the other.
//!
//! Pattern: Adapter. A zone nobody knows (or a custom one) is `Invalid` with the zone code `zone.bad`, never guessed
//! (ADR 0085); the checks run before anything is written.

use chrono::{NaiveDate, NaiveDateTime, TimeZone};
use chrono_tz::Tz;
use fs_tasks_testdata::tasks_v1::{
    Due, Error, LocalDateTime, Problem, TaskError, TaskResult, TypeProblem, Zone, ZoneKind,
};

/// A local date-time as `chrono` value.
#[must_use]
pub fn naive(l: &LocalDateTime) -> Option<NaiveDateTime> {
    l.date()
        .as_naive()
        .and_hms_opt(l.hour().into(), l.minute().into(), l.second().into())
}

/// The tz database entry of a zone; `field` names where the problem is reported (`due.zone`, `view_zone` ...).
///
/// # Errors
/// `Invalid` with the code of the zone check for a custom zone or an unknown name.
pub fn tz(zone: &Zone, field: &str) -> TaskResult<Tz> {
    let bad = || {
        TaskError::from(Error::Invalid(vec![Problem {
            field: field.to_owned(),
            reason: TypeProblem::BadZone.code().to_owned(),
        }]))
    };
    if zone.kind() == ZoneKind::Custom {
        return Err(bad());
    }
    zone.id().parse::<Tz>().map_err(|_| bad())
}

/// A timed due value needs a zone that exists; the other forms need nothing.
///
/// # Errors
/// `Invalid` at `field` (`due.zone`, `start.zone`).
pub fn check_due(due: &Due, field: &str) -> TaskResult<()> {
    if let Due::Timed { zone, .. } = due {
        tz(zone, field)?;
    }
    Ok(())
}

/// The UTC instant of a timed due (`None` for the other forms).
#[must_use]
pub fn instant(due: &Due) -> Option<i64> {
    let Due::Timed { at, zone, .. } = due else {
        return None;
    };
    let zone = tz(zone, "zone").ok()?;
    let local = zone.from_local_datetime(&naive(at)?).earliest()?;
    Some(local.timestamp_millis())
}

/// The day a due value counts for, seen from `view`: a plain date and a floating time by their own day, a timed one by
/// its day in `view`.
#[must_use]
pub fn day_in(due: &Due, view: Tz) -> Option<NaiveDate> {
    match due {
        Due::Date(d) => Some(d.as_naive()),
        Due::Floating(l) => Some(l.date().as_naive()),
        Due::Timed { at, zone, .. } => {
            let own = naive(at)?;
            let Ok(zone) = tz(zone, "zone") else {
                return Some(own.date());
            };
            let at = zone.from_local_datetime(&own).earliest()?;
            Some(at.with_timezone(&view).date_naive())
        }
    }
}
