//! Rules for "repeat from completion" on the awkward days: the end of a month and of a leap year (the day clamps
//! backward), the hour that does not exist in spring, and a plain date or floating time seen in the zone of the person
//! near midnight. The completion day is the day of the fixture's clock, so the rules move it first.
//!
//! Pattern: Template method (rules as plain checks collected in the report). The clock only goes forward, so these
//! rules run last, in order of their dates.

use super::ConformanceFixture;
use super::cx::Cx;
use super::rules_repeat::{finish, rule, series, timed, wall_due};
use crate::sample::wall as wall_of;
use crate::tasks_v1::{Due, Frequency, ObjectId, Progress, RuleEnd, SeriesScope};
use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, Weekday};

fn day(text: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S").ok()
}

/// One case: a series with `due`, the clock at `now` (UTC), completed, the next due must read `want`.
#[allow(clippy::too_many_arguments)]
async fn case(
    fx: &dyn ConformanceFixture,
    cx: &mut Cx<'_>,
    list: ObjectId,
    name: &str,
    due: Due,
    freq: Frequency,
    now: &str,
    want: &str,
) {
    let (Some(now), Some(want)) = (day(now), day(want)) else {
        return;
    };
    let Some(t) = series(
        cx,
        list,
        name,
        Some(due),
        None,
        rule(freq, 1, RuleEnd::Never),
    )
    .await
    else {
        return;
    };
    let ahead = now.and_utc().timestamp_millis() - t.updated.millis() - 2000;
    if ahead < 0 || !fx.advance_clock(ahead).await {
        cx.skip(&format!(
            "repeat dates {name}: the fixture cannot set the clock"
        ));
        return;
    }
    let Ok(next) = finish(cx, &t, Progress::Completed, SeriesScope::Whole).await else {
        cx.ensure(false, &format!("repeat dates {name}: completing answers"));
        return;
    };
    cx.ensure(
        wall_due(&next) == Some(want),
        &format!("repeat dates {name}: the next due is {want}"),
    );
}

/// The dates of the rules, in the year after the one the fixture's clock is in (the clock only goes forward).
struct Dates {
    jan31: NaiveDate,
    gap_day: NaiveDate,
    midnight: NaiveDate,
    leap: NaiveDate,
}

fn dates(year: i32) -> Option<Dates> {
    let mut gap_day = NaiveDate::from_ymd_opt(year, 3, 31)?;
    while gap_day.weekday() != Weekday::Sun {
        gap_day = gap_day.pred_opt()?;
    }
    let leap_year = (year + 1..year + 9).find(|y| NaiveDate::from_ymd_opt(*y, 2, 29).is_some())?;
    Some(Dates {
        jan31: NaiveDate::from_ymd_opt(year, 1, 31)?,
        gap_day,
        midnight: NaiveDate::from_ymd_opt(year, 6, 10)?,
        leap: NaiveDate::from_ymd_opt(leap_year, 2, 29)?,
    })
}

fn at(day: NaiveDate, time: &str) -> String {
    format!("{day}T{time}")
}

pub(super) async fn rules(fx: &dyn ConformanceFixture, cx: &mut Cx<'_>) {
    if !cx.caps("task.progress") {
        return;
    }
    let Some(list) = cx.list("repeat dates").await else {
        return;
    };
    let l = list.id;
    let Some(clock) = cx.task(l, "clock").await else {
        return;
    };
    let year =
        chrono::DateTime::from_timestamp_millis(clock.updated.millis()).map_or(2026, |d| d.year());
    let Some(d) = dates(year + 1) else { return };
    let feb_end = |y: i32| NaiveDate::from_ymd_opt(y, 3, 1).and_then(|m| m.pred_opt());
    let (Some(feb), Some(next_feb)) = (feb_end(d.jan31.year()), feb_end(d.leap.year() + 1)) else {
        return;
    };
    let due = timed(cx, &at(d.jan31 - Duration::days(26), "09:00:00"));
    case(
        fx,
        cx,
        l,
        "monthly from the 31st of January",
        due,
        Frequency::Monthly,
        &at(d.jan31, "10:00:00"),
        &at(feb, "09:00:00"),
    )
    .await;
    let due = timed(cx, &at(d.gap_day - Duration::days(7), "02:30:00"));
    let eve = d.gap_day - Duration::days(1);
    case(
        fx,
        cx,
        l,
        "daily into the hour that does not exist",
        due,
        Frequency::Daily,
        &at(eve, "10:00:00"),
        &at(d.gap_day, "03:30:00"),
    )
    .await;
    let next = d.midnight + Duration::days(2);
    let due = Due::Date(crate::sample::date(&d.midnight.to_string()));
    case(
        fx,
        cx,
        l,
        "date near midnight in Berlin",
        due,
        Frequency::Daily,
        &at(d.midnight, "22:30:00"),
        &at(next, "00:00:00"),
    )
    .await;
    let due = Due::Floating(wall_of(&at(d.midnight - Duration::days(9), "09:00:00")));
    case(
        fx,
        cx,
        l,
        "floating near midnight in Berlin",
        due,
        Frequency::Daily,
        &at(d.midnight + Duration::days(1), "22:30:00"),
        &at(next + Duration::days(1), "09:00:00"),
    )
    .await;
    let due = timed(cx, &at(d.leap - Duration::days(28), "09:00:00"));
    case(
        fx,
        cx,
        l,
        "yearly from the 29th of February",
        due,
        Frequency::Yearly,
        &at(d.leap, "10:00:00"),
        &at(next_feb, "09:00:00"),
    )
    .await;
}
