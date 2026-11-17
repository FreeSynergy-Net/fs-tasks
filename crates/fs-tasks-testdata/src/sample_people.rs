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
    /// An AI (a person) or an AI group: the directory marks it, the groups fake lists it as agent.
    pub ai: bool,
}

/// 1 = the acting person ("Ich"), 2 to 5 = colleagues, 6 and 7 = groups, 8 = an AI, 9 = an AI group. The same people
/// stand in [`crate::FakePeople`] (names, kinds, AI mark) and in `FakeGroupMembers::sample` (who is in which group).
pub const KNOWN: [Known; 9] = [
    Known {
        number: 1,
        display: "Anna Muster",
        address: "anna.muster@example.org",
        art: PersonArt::Person,
        ai: false,
    },
    Known {
        number: 2,
        display: "Ben Beispiel",
        address: "ben.beispiel@example.org",
        art: PersonArt::Person,
        ai: false,
    },
    Known {
        number: 3,
        display: "Clara Probe",
        address: "clara.probe@example.org",
        art: PersonArt::Person,
        ai: false,
    },
    Known {
        number: 4,
        display: "David Demo",
        address: "david.demo@example.org",
        art: PersonArt::Person,
        ai: false,
    },
    Known {
        number: 5,
        display: "Emil Test",
        address: "emil.test@example.org",
        art: PersonArt::Person,
        ai: false,
    },
    Known {
        number: 6,
        display: "Team Vorstand",
        address: "vorstand@example.org",
        art: PersonArt::Group,
        ai: false,
    },
    Known {
        number: 7,
        display: "Team Umzug",
        address: "umzug@example.org",
        art: PersonArt::Group,
        ai: false,
    },
    Known {
        number: 8,
        display: "Hilfe-Assistent",
        address: "assistent@example.org",
        art: PersonArt::Person,
        ai: true,
    },
    Known {
        number: 9,
        display: "KI Helfer",
        address: "helfer@example.org",
        art: PersonArt::Group,
        ai: true,
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
