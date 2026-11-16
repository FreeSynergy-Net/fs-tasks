//! The invented people of the sample (and of a `PersonDirectory` fake a frontend may build from them).
//!
//! Pattern: Fixed data (no real person or organisation; addresses only at `example.org`).

use crate::ids::person;
use crate::tasks_v1::{PersonArt, PersonRef};

/// A person of the sample with the address a directory would show.
#[derive(Clone, Copy, Debug)]
pub struct Known {
    /// The number of the person IRI (`urn:uuid:...`).
    pub number: u64,
    /// Display name (invented).
    pub display: &'static str,
    /// Address at `example.org`.
    pub address: &'static str,
    /// Person or group.
    pub art: PersonArt,
}

/// 1 = the acting person ("Ich"), 2 to 5 = colleagues, 6 = a group.
pub const KNOWN: [Known; 6] = [
    Known {
        number: 1,
        display: "Anna Muster",
        address: "anna.muster@example.org",
        art: PersonArt::Person,
    },
    Known {
        number: 2,
        display: "Ben Beispiel",
        address: "ben.beispiel@example.org",
        art: PersonArt::Person,
    },
    Known {
        number: 3,
        display: "Clara Probe",
        address: "clara.probe@example.org",
        art: PersonArt::Person,
    },
    Known {
        number: 4,
        display: "David Demo",
        address: "david.demo@example.org",
        art: PersonArt::Person,
    },
    Known {
        number: 5,
        display: "Emil Test",
        address: "emil.test@example.org",
        art: PersonArt::Person,
    },
    Known {
        number: 6,
        display: "Team Vorstand",
        address: "vorstand@example.org",
        art: PersonArt::Group,
    },
];

impl Known {
    /// The reference to this person.
    #[must_use]
    pub fn person(&self) -> PersonRef {
        PersonRef {
            art: self.art,
            ..person(self.number)
        }
    }
}
