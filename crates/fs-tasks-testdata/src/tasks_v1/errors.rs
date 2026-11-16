//! Fehler des Themas `tasks`: die gemeinsamen Fehler (`shared::Error`) plus die Ursachen, die nur Aufgaben kennen.
//!
//! Pattern: Value Object (typisierte Fehler, jede Variante hat eine erreichbare Ursache).

use super::{Claim, Error, ObjectId};

/// Fehler einer Methode von `Tasks` oder `TaskExchange`.
#[derive(Clone, Debug)]
pub enum TaskError {
    /// Gemeinsame Fehler: `Unsupported`, `Changed`, `NotFound`, `Forbidden`, `Invalid`, `UnknownZone`, `Unavailable`.
    Common(Error),
    /// `parent` oder eine Abhängigkeit schlösse einen Kreis (`dependency.cycle-check`); die `uid`s des Kreises. Auch bei einem
    /// Kreis über `parent` stehen `uid`s hier (jede Aufgabe hat eine), nicht `ObjectId`s.
    Cycle(Vec<String>),
    /// `progress` mit `Children::Refuse`, aber Teilaufgaben sind offen; nichts geschrieben. Das Frontend fragt und ruft
    /// mit `Same` oder `Leave` erneut.
    OpenSubtasks(Vec<ObjectId>),
    /// Eine andere Person hält die Übernahme noch (`claim`, `release`).
    Claimed(Claim),
}

impl From<Error> for TaskError {
    fn from(e: Error) -> Self {
        Self::Common(e)
    }
}

pub type TaskResult<T> = std::result::Result<T, TaskError>;

/// Das ausdrückliche "nicht möglich" einer nicht überschriebenen Methode.
///
/// # Errors
/// Immer `TaskError::Common(Error::Unsupported)` mit der Fähigkeitskennung.
pub fn not_possible<T>(capability: &str) -> TaskResult<T> {
    Err(TaskError::Common(Error::Unsupported {
        capability: capability.to_owned(),
        reason: "this provider does not offer it".to_owned(),
    }))
}
