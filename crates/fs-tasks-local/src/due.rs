//! A due value in its five columns (`kind`, `date`, `local`, `zone`, `zone_assumed`), written and read back.
//!
//! Pattern: Adapter (value object to a column group and back; the table's `CHECK` holds the same rule once more).

use fs_objects::v5::codec::{
    boolean, damaged, date_to_int, int, int_to_date, int_to_local, local_to_int, opt_text, text,
    zone_of,
};
use fs_tasks_testdata::tasks_v1::{Due, TaskResult};
use turso::Value;

use crate::cur::Cur;
use crate::error::adapt;

/// The five column values of an optional due value, in the order of the table.
pub fn values(due: Option<&Due>) -> TaskResult<[Value; 5]> {
    let none = || {
        [
            Value::Null,
            Value::Null,
            Value::Null,
            Value::Null,
            boolean(false),
        ]
    };
    let Some(due) = due else { return Ok(none()) };
    Ok(match due {
        Due::Date(d) => [
            text("date"),
            int(date_to_int(d)),
            Value::Null,
            Value::Null,
            boolean(false),
        ],
        Due::Timed {
            at,
            zone,
            zone_assumed,
        } => [
            text("timed"),
            Value::Null,
            int(local_to_int(at).map_err(adapt)?),
            opt_text(Some(zone.id())),
            boolean(*zone_assumed),
        ],
        Due::Floating(at) => [
            text("floating"),
            Value::Null,
            int(local_to_int(at).map_err(adapt)?),
            Value::Null,
            boolean(false),
        ],
    })
}

/// Read the next five columns as an optional due value.
pub fn read(cur: &mut Cur<'_>) -> TaskResult<Option<Due>> {
    let kind = cur.opt_text()?;
    let (date, local, zone, assumed) =
        (cur.opt_int()?, cur.opt_int()?, cur.opt_text()?, cur.flag()?);
    let Some(kind) = kind else { return Ok(None) };
    let missing = || adapt(damaged("due value"));
    Ok(Some(match kind.as_str() {
        "date" => Due::Date(int_to_date(date.ok_or_else(missing)?).map_err(adapt)?),
        "timed" => Due::Timed {
            at: int_to_local(local.ok_or_else(missing)?).map_err(adapt)?,
            zone: zone_of(&zone.ok_or_else(missing)?).map_err(adapt)?,
            zone_assumed: assumed,
        },
        "floating" => Due::Floating(int_to_local(local.ok_or_else(missing)?).map_err(adapt)?),
        _ => return Err(missing()),
    }))
}
