//! The owner of a tasks store: the person every write of the store belongs to (fs-objects, `OpenOptions::with_owner`).
//!
//! Pattern: Adapter — the actor of a program is an IRI string; the store wants a [`PersonRef`].

use fs_objects::PersonRef;
use fs_types::{BasicValue as _, PersonArt, Reference};

use crate::{Error, Result};

/// The owner of a store for `actor` (the IRI of the acting person): a person with that IRI, no frozen copy.
///
/// # Errors
/// `Store(Invalid)` when `actor` is no IRI.
pub fn store_owner(actor: &str) -> Result<PersonRef> {
    let id = Reference::parse(actor).map_err(|e| {
        Error::Store(fs_objects::Error::Invalid(format!(
            "the actor {actor:?} is no IRI: {e}"
        )))
    })?;
    Ok(PersonRef {
        id,
        art: PersonArt::Person,
        abzug: None,
    })
}
