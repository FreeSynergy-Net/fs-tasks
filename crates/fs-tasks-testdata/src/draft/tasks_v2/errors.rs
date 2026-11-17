//! Fehler des Themas `tasks`: die gemeinsamen Fehler (`shared::Error`) plus die Ursachen, die nur Aufgaben kennen.
//!
//! Pattern: Value Object (typisierte Fehler, jede Variante hat eine erreichbare Ursache).

use super::{Claim, ColumnKey, Error, ObjectId, Progress, StageKey};

/// Fehler einer Methode von `Tasks` oder `TaskExchange`.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum TaskError {
    /// Gemeinsame Fehler: `Unsupported`, `Changed`, `NotFound`, `Forbidden`, `Invalid` (auch Zonen), `Unavailable`.
    Common(Error),
    /// `parent` oder eine Abhängigkeit schlösse einen Kreis (`dependency.cycle-check`); die `uid`s des Kreises. Auch bei einem
    /// Kreis über `parent` stehen `uid`s hier (jede Aufgabe hat eine), nicht `ObjectId`s.
    Cycle(Vec<String>),
    /// `progress` mit `Children::Refuse`, aber Teilaufgaben sind offen; nichts geschrieben. Das Frontend fragt und ruft
    /// mit `Same` oder `Leave` erneut.
    OpenSubtasks(Vec<ObjectId>),
    /// Eine andere Person hält die Übernahme noch (`claim`, `release`). Wer nach der claim-Regel gar nicht
    /// übernehmen darf, bekommt `Common(Forbidden { task.claim })`, nicht diesen Fehler.
    Claimed(Claim),
    /// `set_columns`: die Spalten brechen eine Regel B1 bis B6; ALLE gefundenen Fälle, nichts geschrieben.
    Columns(Vec<ColumnProblem>),
}

/// Was an einem Spaltensatz falsch ist (Regeln B1 bis B6 in `board.rs`): Code plus Parameter, der Text steht im Programm
/// (de/en, `words.rs`). `KeyBad` kennt nur der Draht (der Schlüssel eines `RemoteTasks`-Aufrufs ist dort Text; ein
/// [`ColumnKey`] in Rust ist schon geprüft).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ColumnProblem {
    /// B1: keine Spalte.
    None,
    /// B1: mehr als `max` Spalten.
    TooMany { max: usize },
    /// Ein Schlüssel an Stelle `index` ist kein `[a-z0-9-]{1,32}`.
    KeyBad { index: usize },
    /// B2: derselbe Schlüssel zweimal.
    KeyTwice { key: ColumnKey },
    /// B3
    TitleEmpty { key: ColumnKey },
    /// B4: kein Stand in der Spalte.
    ProgressEmpty { key: ColumnKey },
    /// B4: derselbe Stand zweimal in einer Spalte.
    ProgressTwice { key: ColumnKey, progress: Progress },
    /// B5
    WipZero { key: ColumnKey },
    /// B6: ein Stand steht in keiner Spalte.
    ProgressMissing { progress: Progress },
    /// B6: dasselbe Paar (Stand, Stufe) in zwei Spalten (Problem-Typ `column-pair-twice`).
    PairTwice {
        progress: Progress,
        stage: Option<StageKey>,
        first: ColumnKey,
        second: ColumnKey,
    },
}

impl ColumnProblem {
    /// Der Code (Teil des Problem-Typs `https://freesynergy.net/problems/tasks/<code>`, RFC 9457).
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::ProgressMissing { .. } => "columns-incomplete",
            Self::PairTwice { .. } => "column-pair-twice",
            _ => "columns-invalid",
        }
    }
}

/// Die `Invalid`-Codes (`Problem { field, reason }`) dieses Deltas, zusätzlich zu denen von v1; Code `<feld>.<grund>`.
/// Dazu der Problem-Typ `agent-order` für `Forbidden { task.place }` einer KI (Regel KI-ORDNUNG).
pub const INVALID_CODES: [&str; 7] = [
    "stage.bad",
    "note.too-long",
    "note.control",
    "column.unknown",
    "after.outside",
    "lists.empty-for-rank",
    "parent.missing-for-rank",
];

impl From<Error> for TaskError {
    fn from(e: Error) -> Self {
        Self::Common(e)
    }
}

pub type TaskResult<T> = std::result::Result<T, TaskError>;

/// Das ausdrückliche "nicht möglich" eines Anbieters, der eine Fähigkeit nicht hat (er ruft es selbst auf; der Trait
/// hat keine Standardmethoden, damit kein Anbieter eine Methode still vergisst).
///
/// # Errors
/// Immer `TaskError::Common(Error::Unsupported)` mit der Fähigkeitskennung.
pub fn not_possible<T>(capability: &str) -> TaskResult<T> {
    Err(TaskError::Common(Error::Unsupported {
        capability: capability.to_owned(),
        reason: "this provider does not offer it".to_owned(),
    }))
}
