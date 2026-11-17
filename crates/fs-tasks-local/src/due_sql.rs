//! The day range of a query as a condition on the derived day columns (`tasks_day`, see `day.rs`): the database decides, the
//! tasks are not loaded to be looked at. A plain date and a floating time are compared by their own day; a timed value by its
//! instant against the first instant of a day in the viewer's zone.
//!
//! Pattern: Specification (a small function builds the condition text and its parameters in one go; the same answer as
//! `zones::day_in` on the loaded task, which the tests hold against it).

use chrono::{DateTime, Days, NaiveDate};
use chrono_tz::Tz;
use fs_objects::v5::codec::int;
use fs_tasks_testdata::tasks_v1::{DateField, DueRange};
use turso::Value;

/// Days since 1970-01-01 (the storage of a date column).
fn days_of(d: NaiveDate) -> i64 {
    d.signed_duration_since(NaiveDate::from_ymd_opt(1970, 1, 1).unwrap_or(NaiveDate::MIN))
        .num_days()
}

/// The day of the instant `secs` seen from `view`.
fn day_at(view: Tz, secs: i64) -> Option<NaiveDate> {
    Some(
        DateTime::from_timestamp(secs, 0)?
            .with_timezone(&view)
            .date_naive(),
    )
}

/// The first instant (milliseconds) whose day in `view` is `day` or later: the day `day` begins there (a day whose
/// midnight does not exist, a zone's skipped hour, begins at the end of the gap). Found by halving, so the answer is the
/// zone rules' own and not a guess; years 0001 to 9999 and a margin are searched.
#[must_use]
pub fn first_instant_of(view: Tz, day: NaiveDate) -> i64 {
    let (mut lo, mut hi) = (
        -62_135_596_800_i64 - 2 * 86_400,
        253_402_300_799_i64 + 2 * 86_400,
    );
    if day_at(view, lo).is_none_or(|d| d >= day) {
        return lo.saturating_mul(1000);
    }
    if day_at(view, hi).is_none_or(|d| d < day) {
        return hi.saturating_add(1).saturating_mul(1000);
    }
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if day_at(view, mid).is_some_and(|d| d >= day) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    hi * 1000
}

/// One bound of one field: the text of the test and the two values for its marks (the day, the instant).
type Bound = Option<(String, [Value; 2])>;

/// The parts of one field (`due` or `start`): its day is not before `from` (`ge`) and not after `to` (`le`).
fn bounds(field: &str, from: Option<NaiveDate>, to: Option<NaiveDate>, view: Tz) -> (Bound, Bound) {
    let ge = from.map(|f| {
        (
            format!("COALESCE(d.{field}_day >= ?, d.{field}_utc >= ?)"),
            [int(days_of(f)), int(first_instant_of(view, f))],
        )
    });
    let le = to.map(|t| {
        // After `to`: the day is `to + 1` or later, so the instant is at or after its first instant.
        let after = t.checked_add_days(Days::new(1));
        (
            format!("COALESCE(d.{field}_day <= ?, d.{field}_utc < ?)"),
            [
                int(days_of(t)),
                after.map_or(int(i64::MAX), |a| int(first_instant_of(view, a))),
            ],
        )
    });
    (ge, le)
}

/// The condition (over the alias `t` of the task) and its parameters, in the order of their marks.
#[must_use]
pub fn condition(r: &DueRange, view: Tz) -> (String, Vec<Value>) {
    let known = |f: &str| format!("(d.{f}_day IS NOT NULL OR d.{f}_utc IS NOT NULL)");
    let (from, to) = (r.from.map(|d| d.as_naive()), r.to.map(|d| d.as_naive()));
    let mut params = Vec::new();
    let mut put = |b: Bound, wrap: &str| -> Option<String> {
        b.map(|(text, values)| {
            params.extend(values);
            wrap.replace("{}", &text)
        })
    };
    let body = match r.field {
        DateField::Due | DateField::Start => {
            let f = if r.field == DateField::Due {
                "due"
            } else {
                "start"
            };
            let (ge, le) = bounds(f, from, to, view);
            [Some(known(f)), put(ge, "{}"), put(le, "{}")]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" AND ")
        }
        DateField::DueOrStart => {
            // The day of a task is the earlier one of its due and its start day: all that exist are at or after `from`,
            // and at least one is at or before `to`.
            let (ge_d, le_d) = bounds("due", from, to, view);
            let (ge_s, le_s) = bounds("start", from, to, view);
            let mut parts = vec![format!("({} OR {})", known("due"), known("start"))];
            parts.extend(put(ge_d, "COALESCE({}, 1)"));
            parts.extend(put(ge_s, "COALESCE({}, 1)"));
            let (a, b) = (put(le_d, "COALESCE({}, 0)"), put(le_s, "COALESCE({}, 0)"));
            parts.extend(a.zip(b).map(|(a, b)| format!("({a} OR {b})")));
            parts.join(" AND ")
        }
    };
    (
        format!("EXISTS (SELECT 1 FROM tasks_day d WHERE d.task = t.id AND {body})"),
        params,
    )
}
