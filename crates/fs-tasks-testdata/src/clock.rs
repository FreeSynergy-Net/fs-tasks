//! Time helpers of the fake: `fs-types` values to `chrono` arithmetic and back, days in a view zone.
//!
//! Pattern: Adapter (the typed values of the trait on one side, `chrono` and the tz database on the other).

use crate::tasks_v1::{
    Date, Due, Error, LocalDateTime, Problem, TaskError, TaskResult, TypeProblem, Zone, ZoneKind,
};
use chrono::{Duration, NaiveDate, NaiveDateTime, TimeZone, Timelike};
use chrono_tz::Tz;
use fs_types::BasicValue;

/// A local date-time as `chrono` value.
pub(crate) fn naive(l: &LocalDateTime) -> Option<NaiveDateTime> {
    l.date()
        .as_naive()
        .and_hms_opt(l.hour().into(), l.minute().into(), l.second().into())
}

/// A `chrono` value as local date-time.
pub(crate) fn local(n: NaiveDateTime) -> Option<LocalDateTime> {
    let hour = u8::try_from(n.hour()).ok()?;
    let minute = u8::try_from(n.minute()).ok()?;
    let second = u8::try_from(n.second()).ok()?;
    LocalDateTime::new(date_of(n.date())?, hour, minute, second).ok()
}

/// A `chrono` day as typed date.
pub(crate) fn date_of(d: NaiveDate) -> Option<Date> {
    Date::parse(&d.format("%Y-%m-%d").to_string()).ok()
}

/// The tz database entry of a zone; a custom zone or an unknown name is `Invalid` with the zone code (`zone.bad`, never
/// guessed, ADR 0085).
pub(crate) fn tz(zone: &Zone) -> TaskResult<Tz> {
    let bad = || {
        TaskError::from(Error::Invalid(vec![Problem {
            field: "zone".to_owned(),
            reason: TypeProblem::BadZone.code().to_owned(),
        }]))
    };
    if zone.kind() == ZoneKind::Custom {
        return Err(bad());
    }
    zone.id().parse::<Tz>().map_err(|_| bad())
}

/// Wall-clock reading of a due value (a plain date reads as midnight).
pub(crate) fn wall(due: &Due) -> Option<NaiveDateTime> {
    match due {
        Due::Date(d) => d.as_naive().and_hms_opt(0, 0, 0),
        Due::Timed { at, .. } | Due::Floating(at) => naive(at),
    }
}

/// The UTC instant of a timed due (`None` for the other forms and for an unknown zone).
pub(crate) fn instant(due: &Due) -> Option<i64> {
    let Due::Timed { at, zone, .. } = due else {
        return None;
    };
    let zone = tz(zone).ok()?;
    let local = zone.from_local_datetime(&naive(at)?).earliest()?;
    Some(local.timestamp_millis())
}

/// The day a due value counts for, seen from `view`: a plain date and a floating time by their own day, a timed
/// one by its day in `view`.
pub(crate) fn day_in(due: &Due, view: Tz) -> Option<NaiveDate> {
    match due {
        Due::Date(d) => Some(d.as_naive()),
        Due::Floating(l) => Some(l.date().as_naive()),
        Due::Timed { at, zone, .. } => {
            let own = naive(at)?;
            let Ok(zone) = tz(zone) else {
                return Some(own.date());
            };
            let at = zone.from_local_datetime(&own).earliest()?;
            Some(at.with_timezone(&view).date_naive())
        }
    }
}

/// Move a due value by the distance between two wall-clock readings (an occurrence of a series).
pub(crate) fn shift(due: &Due, from: NaiveDateTime, to: NaiveDateTime) -> Due {
    let delta: Duration = to - from;
    match due {
        Due::Date(d) => date_of(d.as_naive() + Duration::days(delta.num_days()))
            .map_or_else(|| due.clone(), Due::Date),
        Due::Timed {
            at,
            zone,
            zone_assumed,
        } => naive(at).and_then(|n| local(n + delta)).map_or_else(
            || due.clone(),
            |at| Due::Timed {
                at,
                zone: zone.clone(),
                zone_assumed: *zone_assumed,
            },
        ),
        Due::Floating(at) => naive(at)
            .and_then(|n| local(n + delta))
            .map_or_else(|| due.clone(), Due::Floating),
    }
}
