//! Deterministic ids and small error helpers, so a test can name a thing without asking the provider.
//!
//! Pattern: none (pure functions).

use crate::tasks_v1::{ObjectId, PersonArt, PersonRef, Problem, Reference, TaskError};
use fs_types::BasicValue;

/// The object id with the number `n` (big-endian in the last eight bytes).
#[must_use]
pub fn object_id(n: u64) -> ObjectId {
    let mut bytes = [0_u8; 16];
    bytes[8..].copy_from_slice(&n.to_be_bytes());
    ObjectId(bytes)
}

/// The number an [`object_id`] was made from.
#[must_use]
pub fn object_number(id: ObjectId) -> u64 {
    let mut tail = [0_u8; 8];
    tail.copy_from_slice(&id.0[8..]);
    u64::from_be_bytes(tail)
}

/// A reference from text, `None` when `text` is not an absolute IRI.
#[must_use]
pub fn try_reference(text: &str) -> Option<Reference> {
    Reference::parse(text).ok()
}

/// A reference from text.
///
/// # Panics
/// When `text` is not an absolute IRI (a mistake in the test data, never input of a provider); [`try_reference`]
/// does not panic.
#[must_use]
pub fn reference(text: &str) -> Reference {
    try_reference(text).expect("a reference of the test data")
}

/// A reference to the person with the number `n` (kind `Person`, no excerpt).
#[must_use]
pub fn person(n: u64) -> PersonRef {
    PersonRef {
        id: reference(&format!("urn:uuid:00000000-0000-7000-8000-{n:012x}")),
        art: PersonArt::Person,
        abzug: None,
    }
}

/// The `Invalid` error of one field (`reason` is a code, ADR 0057).
pub(crate) fn invalid(field: &str, reason: &str) -> TaskError {
    TaskError::Common(crate::tasks_v1::Error::Invalid(vec![Problem {
        field: field.to_owned(),
        reason: reason.to_owned(),
    }]))
}
