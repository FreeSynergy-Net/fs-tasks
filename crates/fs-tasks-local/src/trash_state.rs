//! What the envelope says about the trash, as the trait says it: `Restorable`, computed here.
//!
//! Pattern: Value Object (a deadline pair and the pure function from it to `Restorable`). fs-objects v0.0.2 has no
//! `restorable_until`, no hold and no "never automatically" (those are v0.0.3, `object_hold`, and not part of this
//! pin): the trash period is `setting.trash_days` (default 30, at least 1), the deadline of an object is the
//! `purge_after` of its envelope, and `Restorable::Until` is that deadline. `Held` and `Manual` are run 2, with the
//! lift of the pin.

use fs_tasks_testdata::tasks_v1::{Restorable, Utc};

/// When something was deleted and until when it can come back (both in milliseconds).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gone {
    pub deleted: i64,
    pub until: i64,
}

impl Gone {
    /// The trait's view of the deadline: restorable until the end of the trash period.
    #[must_use]
    pub fn restorable(self) -> Restorable {
        Restorable::Until(Utc::from_millis(self.until))
    }
}

/// The pair of columns `deleted`, `purge_after` of an envelope row (both empty or both set, by the table's `CHECK`).
#[must_use]
pub fn gone(deleted: Option<i64>, until: Option<i64>) -> Option<Gone> {
    Some(Gone {
        deleted: deleted?,
        until: until?,
    })
}
