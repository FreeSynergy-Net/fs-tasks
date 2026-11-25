//! The source of the owner in the shipped program: the person "me" of the module `person`.
//!
//! Pattern: Factory Method — builds the `LocalOwner` (Strategy) of the shipped binary: the provider `PersonServiceOwner` of the
//! module person (ADR 0104 E9: a module asks another module through its contract; it gets or starts its one service). Where the
//! program cannot tell where that service is, the source says so (`Unreachable`) and the service does not start. No fake lives
//! here: the test owners are in the feature `fake-owner` and in the dev-dependencies, never in a build of the program.

use std::path::Path;

use fs_interfaces::owner::{LocalOwner, OwnerProblem};
use fs_objects::PersonRef;
use fs_person_service::PersonServiceOwner;

/// The owner source of the program that runs as `running` (an absolute path): the service of the module person on its usual
/// socket, started from the program file next to `running` or in `/usr/bin` (never found on `PATH`).
#[must_use]
pub fn person_owner(running: &Path) -> Box<dyn LocalOwner> {
    match PersonServiceOwner::standard(running) {
        Some(owner) => Box::new(owner),
        None => Box::new(NoPlace),
    }
}

/// The program knows neither its own place nor a place for the socket: the module person cannot be asked.
struct NoPlace;

impl LocalOwner for NoPlace {
    fn owner(&self) -> Result<PersonRef, OwnerProblem> {
        Err(OwnerProblem::Unreachable {
            reason: "the place of the module person is unknown (no absolute path of this program, or no place for the socket)".into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A relative path of the program gives no place to look for the service: it is told, never guessed.
    #[test]
    fn a_program_without_an_absolute_path_cannot_ask_the_module_person() {
        let got = person_owner(Path::new("fs-tasks")).owner();
        let Err(OwnerProblem::Unreachable { reason }) = got else {
            panic!("{got:?}");
        };
        assert!(reason.contains("module person"), "{reason}");
    }
}
