//! Errors: the layer's error in the trait's, and the one-field answers the operations give.
//!
//! Pattern: Adapter (one function per direction; no state). The layer speaks `fs-types`, so only the error needs
//! mapping; a cause the trait has no word for is `Unavailable` with the layer's text.

use fs_objects::v5;
use fs_tasks_testdata::tasks_v1::{Error, ObjectId, Problem, Revision, TaskError};

/// The layer's error as the trait's.
#[must_use]
pub fn adapt(e: v5::Error) -> TaskError {
    TaskError::Common(match e {
        v5::Error::NotFound(id) => Error::NotFound(id),
        v5::Error::Invalid(problems) => Error::Invalid(problems),
        v5::Error::Changed { current } => Error::Changed { current },
        other => Error::Unavailable(other.to_string()),
    })
}

/// `Invalid` at one field; `reason` is a code (ADR 0057), never a sentence.
#[must_use]
pub fn bad(field: &str, reason: &str) -> TaskError {
    TaskError::Common(Error::Invalid(vec![Problem {
        field: field.to_owned(),
        reason: reason.to_owned(),
    }]))
}

/// `NotFound` for an object that is not there (or may not be seen).
#[must_use]
pub fn gone(id: ObjectId) -> TaskError {
    TaskError::Common(Error::NotFound(id))
}

/// `Unsupported` for a field or a form this run does not store yet (named in `stand.md`): never a silent drop.
#[must_use]
pub fn unsupported(capability: &str, reason: &str) -> TaskError {
    TaskError::Common(Error::Unsupported {
        capability: capability.to_owned(),
        reason: reason.to_owned(),
    })
}

/// `Forbidden` with the capability that was refused.
#[must_use]
pub fn forbidden(capability: &str) -> TaskError {
    TaskError::Common(Error::Forbidden {
        capability: capability.to_owned(),
    })
}

/// Compare-and-set lost: `current` is what the row says now.
#[must_use]
pub fn changed(current: Revision) -> TaskError {
    TaskError::Common(Error::Changed { current })
}
