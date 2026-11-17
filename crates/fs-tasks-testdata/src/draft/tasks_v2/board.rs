//! Board-Spalten einer Liste: Schlüssel, Spalte, Spaltensatz, Regeln B1 bis B7, Platz beim Einfügen.
//!
//! Pattern: Value Object + Specification (Schlüssel nur über `parse`; `problems` und `column_of` sind reine
//! Funktionen, die jeder Anbieter UND jedes Frontend gleich rechnet; es gibt keine Board-Objekte und keine Kartenplätze,
//! die Reihenfolge ist `Task::rank`, ADR 0049 Nachtrag 2026-11-17).

use super::{ColumnProblem, ObjectId, Progress, Revision};
use std::collections::HashMap;

/// Höchstzahl der Spalten einer Liste (B1).
pub const COLUMNS_MAX: usize = 32;

/// Prüft `[a-z0-9-]{1,32}`.
fn key_ok(text: &str) -> bool {
    (1..=32).contains(&text.len())
        && text
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Unterstufe innerhalb eines `progress` (`[a-z0-9-]{1,32}`); `None` an der Aufgabe = Grundstufe. Speicher: `TEXT NOT
/// NULL`, `''` = `None`.
///
/// ```
/// use fs_tasks_testdata::draft::tasks_v2::StageKey;
/// assert_eq!(StageKey::parse("testing").unwrap().as_str(), "testing");
/// assert!(StageKey::parse("Testing").is_none() && StageKey::parse("").is_none());
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StageKey(String);

/// Stabiler Schlüssel einer Spalte (gleiche Syntax; Umbenennen ändert nur `BoardColumn::title`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ColumnKey(String);

macro_rules! key_type {
    ($name:ident) => {
        impl $name {
            /// Prüft den Text (`[a-z0-9-]{1,32}`); sonst `None`.
            #[must_use]
            pub fn parse(text: &str) -> Option<Self> {
                key_ok(text).then(|| Self(text.to_owned()))
            }
            /// Der Text (für eine typisierte Textspalte).
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}
key_type!(StageKey);
key_type!(ColumnKey);

/// Eine Spalte. Ihre Paare sind `(p, stage)` für jedes `p` in `progress`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoardColumn {
    pub key: ColumnKey,
    pub title: String,
    /// Mindestens ein Wert. Der ERSTE wird beim Hineinziehen geschrieben, die übrigen werden nur angezeigt (etwa
    /// `Geschlossen` = gescheitert + abgebrochen).
    pub progress: Vec<Progress>,
    pub stage: Option<StageKey>,
    /// Hinweis, nie Sperre (b7 Frage 3); `None` = unbegrenzt; `Some(0)` ist ein Fehler (B5).
    pub wip: Option<u32>,
}

/// Wohin in einer Spalte eingefügt wird: ganz oben, ganz unten oder direkt nach einer Aufgabe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Top,
    Bottom,
    After(ObjectId),
}

/// Rand beim Anlegen (`TaskDraft::at`): ohne Bezugsaufgabe nur `Top` oder `Bottom`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Edge {
    Top,
    #[default]
    Bottom,
}

/// Spalten einer Liste, links nach rechts (Position = Index). `stored = false`: keine Einstellung gespeichert, der
/// Standard wird geliefert. `revision` gehört dem Satz (eigene Spalte je Liste), nicht der Liste.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColumnSet {
    pub list: ObjectId,
    pub revision: Revision,
    pub stored: bool,
    pub columns: Vec<BoardColumn>,
}

fn column(key: &str, title: &str, progress: &[Progress]) -> BoardColumn {
    BoardColumn {
        key: ColumnKey::parse(key).expect("a fixed key"),
        title: title.to_owned(),
        progress: progress.to_vec(),
        stage: None,
        wip: None,
    }
}

impl ColumnSet {
    /// Die vier Spalten von heute (`stored = false`): Offen, In Arbeit, Fertig, Geschlossen (Schlüssel
    /// `todo|doing|done|closed`).
    ///
    /// ```
    /// use fs_tasks_testdata::draft::tasks_v2::{ColumnSet, ObjectId, Revision};
    /// let set = ColumnSet::standard(ObjectId([1; 16]), Revision(0));
    /// assert_eq!(set.columns.len(), 4);
    /// assert!(!set.stored && set.problems().is_empty());
    /// ```
    #[must_use]
    pub fn standard(list: ObjectId, revision: Revision) -> Self {
        use Progress::{Cancelled, Completed, Failed, InProcess, NeedsAction};
        Self {
            list,
            revision,
            stored: false,
            columns: vec![
                column("todo", "Offen", &[NeedsAction]),
                column("doing", "In Arbeit", &[InProcess]),
                column("done", "Fertig", &[Completed]),
                column("closed", "Geschlossen", &[Failed, Cancelled]),
            ],
        }
    }

    /// Regeln B1 bis B6; leer = gültig; ALLE Verstöße, nie nur der erste. B1 mindestens 1, höchstens [`COLUMNS_MAX`]
    /// Spalten. B2 `key` eindeutig. B3 `title` nicht leer (getrimmt). B4 `progress` nicht leer, kein Wert doppelt. B5
    /// `wip != Some(0)`. B6 VOLLSTÄNDIG UND EINDEUTIG: jeder der fünf Werte steht in mindestens einer Spalte, und jedes
    /// Paar `(progress, stage)` steht über alle Spalten höchstens einmal.
    #[must_use]
    pub fn problems(&self) -> Vec<ColumnProblem> {
        let mut out = Vec::new();
        if self.columns.is_empty() {
            out.push(ColumnProblem::None);
        }
        if self.columns.len() > COLUMNS_MAX {
            out.push(ColumnProblem::TooMany { max: COLUMNS_MAX });
        }
        let mut keys: Vec<&ColumnKey> = Vec::new();
        let mut pairs: HashMap<(Progress, Option<&super::StageKey>), &ColumnKey> = HashMap::new();
        for c in &self.columns {
            if keys.contains(&&c.key) {
                out.push(ColumnProblem::KeyTwice { key: c.key.clone() });
            }
            keys.push(&c.key);
            if c.title.trim().is_empty() {
                out.push(ColumnProblem::TitleEmpty { key: c.key.clone() });
            }
            if c.progress.is_empty() {
                out.push(ColumnProblem::ProgressEmpty { key: c.key.clone() });
            }
            if c.wip == Some(0) {
                out.push(ColumnProblem::WipZero { key: c.key.clone() });
            }
            for (i, p) in c.progress.iter().enumerate() {
                if c.progress[..i].contains(p) {
                    let key = c.key.clone();
                    out.push(ColumnProblem::ProgressTwice { key, progress: *p });
                } else if let Some(first) = pairs.insert((*p, c.stage.as_ref()), &c.key) {
                    out.push(ColumnProblem::PairTwice {
                        progress: *p,
                        stage: c.stage.clone(),
                        first: first.clone(),
                        second: c.key.clone(),
                    });
                }
            }
        }
        for p in Progress::ALL {
            if !self.columns.iter().any(|c| c.progress.contains(&p)) {
                out.push(ColumnProblem::ProgressMissing { progress: p });
            }
        }
        out
    }

    /// Regel B7, PLATZ EINER KARTE (nie unsichtbar): die Spalte mit genau `(progress, stage)`; sonst die mit
    /// `(progress, None)`; sonst die erste Spalte (links), die `progress` enthält. Eine Karte, deren `stage` keine eigene
    /// Spalte hat, trägt im Programm die Marke „Stufe ‹s› ohne Spalte“. `None` gibt es nur bei einem Satz, der B6
    /// verletzt (also nie bei einem gespeicherten).
    #[must_use]
    pub fn column_of(
        &self,
        progress: Progress,
        stage: Option<&super::StageKey>,
    ) -> Option<&ColumnKey> {
        let has = |c: &&BoardColumn| c.progress.contains(&progress);
        let exact = self.columns.iter().filter(has).find(|c| c.stage.as_ref() == stage);
        let base = || self.columns.iter().filter(has).find(|c| c.stage.is_none());
        let first = || self.columns.iter().find(has);
        exact.or_else(base).or_else(first).map(|c| &c.key)
    }
}
