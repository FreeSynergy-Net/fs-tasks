//! Die Fassung des Anbieters für die Fälle: Personen jeder Art, Karten in Spalten, Fehler einspritzen, Neustart.
//!
//! Pattern: Strategy / Test fixture (jeder Anbieter sagt, wie man ihn in die Zustände der Fälle bringt; hier nur die
//! Form, ohne Anbieter: alles `unimplemented!`, bis der Kern da ist).

use crate::draft::tasks_v2::shared::{ObjectId, PersonRef};
use crate::draft::tasks_v2::{
    BoardColumn, ColumnKey, Progress, Task, TaskDraft, TaskPatch, TaskQuery, Tasks,
};

/// Grund für `unimplemented!`: ein ignorierter Fall darf nie still grün werden.
const KERN: &str = "kern: the v2 provider (FakeTasks v2 / LocalTasks) does not exist yet";

/// Was der Anbieter an Fehlern einspritzen können muss (wie `FailingTasks`/`SlowTasks` in v1).
#[derive(Clone, Copy, Debug)]
pub(super) enum Fault {
    /// Abbruch nach dem Schreiben der Aufgabe, vor dem Zähler (K29, `FailingTasks`).
    BeforeCounter,
}

/// Der Anbieter unter Test plus die Dinge, die der Trait nicht kann.
pub(super) struct Kern {
    pub(super) t: Box<dyn Tasks>,
}

impl Kern {
    pub(super) fn start() -> Self {
        unimplemented!("{KERN}")
    }
    /// Neue Person der Art (Mensch, KI, KI-Status unbekannt).
    pub(super) fn human(&self) -> PersonRef {
        unimplemented!("{KERN}")
    }
    pub(super) fn agent(&self) -> PersonRef {
        unimplemented!("{KERN}")
    }
    pub(super) fn unknown(&self) -> PersonRef {
        unimplemented!("{KERN}")
    }
    /// Die fragende Person wechseln (Eigenschaft des Anbieters).
    pub(super) fn act(&self, _who: &PersonRef) {
        unimplemented!("{KERN}")
    }
    /// Der Anbieter wird neu geöffnet auf demselben Speicher (K28) oder aus einer Sicherung wiederhergestellt (K30).
    pub(super) fn reopen(&mut self) {
        unimplemented!("{KERN}")
    }
    pub(super) fn restore_backup(&mut self) {
        unimplemented!("{KERN}")
    }
    pub(super) fn fail(&self, _fault: Fault) {
        unimplemented!("{KERN}")
    }
    /// Eine Liste der fragenden Person (sie ist `Manage`).
    pub(super) async fn list(&self) -> ObjectId {
        unimplemented!("{KERN}")
    }
    /// Eine Abfrage mit gesetzter Zone, sonst leer, `order = Rank`.
    pub(super) fn query(&self, _lists: Vec<ObjectId>) -> TaskQuery {
        unimplemented!("{KERN}")
    }
    /// Altbestand wie vor v2 (K23): Aufgaben ohne `stage`, `note` im Herstellerfeld, keine Spalten-Zeilen.
    pub(super) async fn legacy_list(&self) -> ObjectId {
        unimplemented!("{KERN}")
    }
    /// Die Ids der Karten einer Folge (Liste, Spalte nach B7), von oben nach unten.
    pub(super) async fn order_of(&self, _list: ObjectId, _column: &ColumnKey) -> Vec<ObjectId> {
        unimplemented!("{KERN}")
    }
    /// Alle `Change` seit Beginn, in Reihenfolge.
    pub(super) async fn changes(&self) -> Vec<crate::draft::tasks_v2::Change> {
        unimplemented!("{KERN}")
    }
    /// Eine Karte in `progress` am Ende der Folge (als fragende Person angelegt).
    pub(super) async fn card(&self, list: ObjectId, progress: Progress) -> Task {
        let mut d = draft(list, "card");
        d.progress = progress;
        self.t.create(d).await.expect("create a card")
    }
    /// Die Vorlage „Entwicklung“ (Entwurf Abschnitt 6) in die Liste schreiben.
    pub(super) async fn dev_columns(&self, _list: ObjectId) -> Vec<BoardColumn> {
        unimplemented!("{KERN}")
    }
}

/// Ein Entwurf mit den Standardwerten (Stand `NeedsAction`, ohne Stufe, `Bottom`).
pub(super) fn draft(list: ObjectId, title: &str) -> TaskDraft {
    TaskDraft {
        list,
        uid: None,
        title: title.to_owned(),
        progress: Progress::NeedsAction,
        assignees: Vec::new(),
        checklist: Vec::new(),
        stage: None,
        at: crate::draft::tasks_v2::Edge::default(),
        rest: TaskPatch::default(),
    }
}

/// Eine Spalte (Schlüssel und Stufe als Text, beide gültig).
pub(super) fn col(key: &str, progress: &[Progress], stage: Option<&str>) -> BoardColumn {
    BoardColumn {
        key: ColumnKey::parse(key).expect("a valid key"),
        title: key.to_owned(),
        progress: progress.to_vec(),
        stage: stage.map(|s| crate::draft::tasks_v2::StageKey::parse(s).expect("a valid stage")),
        wip: None,
    }
}

/// Der Standard als Wert (4 Spalten wie `ColumnSet::standard`), zum Abwandeln.
pub(super) fn std4() -> Vec<BoardColumn> {
    use Progress::{Cancelled, Completed, Failed, InProcess, NeedsAction};
    vec![
        col("todo", &[NeedsAction], None),
        col("doing", &[InProcess], None),
        col("done", &[Completed], None),
        col("closed", &[Failed, Cancelled], None),
    ]
}

/// Ob unter den `Invalid`-Problemen der Code `<feld>.<grund>` steht (Codes: `INVALID_CODES`).
pub(super) fn has_code(problems: &[crate::draft::tasks_v2::shared::Problem], code: &str) -> bool {
    problems
        .iter()
        .any(|p| format!("{}.{}", p.field, p.reason) == code)
}
