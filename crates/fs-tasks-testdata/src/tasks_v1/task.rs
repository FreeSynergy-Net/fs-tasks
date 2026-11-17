//! Aufgabe, Entwurf, Änderung, Ausnahme und Teile (die Liste steht in `list.rs`).
//!
//! Pattern: Value Object (Aufgabe und Teile als reine Daten, je Liste eine Untertabelle).

use super::{
    ChecklistItem, Due, FileRef, LocalDateTime, MediaType, ObjectId, Origin, PersonRef, Rank,
    Recurrence, RepeatFrom, Revision, Span, Utc,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextType {
    Plain,
    Html,
}
/// Stand einer Aufgabe (`JSCalendar` `progress`). Die Board-Spalten sind diese Werte (ADR 0049: Kanban ist Ansicht).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Progress {
    NeedsAction,
    InProcess,
    Completed,
    Failed,
    Cancelled,
}
/// Antwort einer zugewiesenen Person ("annehmen", "abweisen"); `NeedsAction` = noch keine Antwort.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AssigneeStatus {
    NeedsAction,
    Accepted,
    Declined,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Answer {
    Accept,
    Decline,
}
/// Abhängigkeit nach RFC 9253 (`RELTYPE`): die Aufgabe, die sie hält, ist bei den vier Zeitarten der Vorgänger; bei
/// `DependsOn` die abhängige.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DependencyKind {
    FinishToStart,
    FinishToFinish,
    StartToStart,
    StartToFinish,
    DependsOn,
}
/// Struktur-Beziehung einer geteilten Serie (IANA `first`/`next`). Eltern/Kind steht in `parent`, nicht hier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Relation {
    First,
    Next,
}
/// Was mit offenen Teilaufgaben geschieht, wenn die Eltern-Aufgabe erledigt, abgebrochen oder als gescheitert gesetzt
/// wird (Rückweg der Frage im Frontend, `choice-dialog`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Children {
    /// Nichts ändern, wenn Teilaufgaben offen sind: `TaskError::OpenSubtasks` mit ihrer Liste.
    Refuse,
    /// Die offenen Teilaufgaben bekommen denselben Stand.
    Same,
    /// Die Teilaufgaben bleiben, wie sie sind.
    Leave,
}

/// Eine zuständige Person oder Gruppe (`JSCalendar` `participants` mit Rolle `attendee`; eine Gruppe mit `kind`
/// `group`). Kein Namensfeld (ADR 0085). `assigned_by`/`assigned`: wer wann zugewiesen hat; `None` nur bei
/// Importiertem, wo die Quelle es nicht sagt. Bei einer Gruppe bleibt `status` `NeedsAction` (eine Gruppe antwortet
/// nicht; ein Mitglied übernimmt mit `claim`, siehe `mod.rs`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assignee {
    pub who: PersonRef,
    pub status: AssigneeStatus,
    pub assigned_by: Option<PersonRef>,
    pub assigned: Option<Utc>,
    pub comment: Option<String>,
}

/// Übernahme gegen doppeltes Bearbeiten (Menschen und markierte KIs, ADR 0054), bis `until`; `by` ist immer eine
/// Person, nie eine Gruppe (claim-Regel in `mod.rs`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Claim {
    pub by: PersonRef,
    pub until: Utc,
}

/// Abstand einer Abhängigkeit (RFC 9253 `GAP`): `negative` = Vorlauf.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Gap {
    pub negative: bool,
    pub span: Span,
}
/// Abhängigkeit zu einer anderen Aufgabe per `uid` (auch fremd; fehlt sie, zeigt `Readiness::Unknown` es).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dependency {
    pub other_uid: String,
    pub kind: DependencyKind,
    /// Nur bei den vier Zeitarten; `Some` bei `DependsOn` ist `Invalid` (RFC 9253: `DEPENDS-ON` kennt kein `GAP`).
    pub gap: Option<Gap>,
}
/// Beziehung einer geteilten Serie per `uid`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Related {
    pub uid: String,
    pub relation: Relation,
}

// Erinnerung (`alerts`) kommt aus fs-jscalendar (Entscheidung 2026-11-16: nicht kopieren). `RelativeTo::End` heißt bei einer
// Aufgabe: bezogen auf `due` (RFC 8984 `relativeTo: "end"`; eine Aufgabe hat kein Ende). Bei `Due::Date` ohne Uhrzeit
// rechnet das Gerät den Tag in seiner Zone (wie `Floating`).
// `recurrence` und `excluded_recurrence` sind `Recurrence` (Regel oder erhaltener Original-Text, ADR 0030).
pub use fs_jscalendar::alert::{Alert, AlertAction, RelativeTo, Trigger};
/// Anhang oder Verweis: eine Datei bei uns (`file`, fs-files) oder eine fremde Adresse (`href`); genau eins von beiden.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub key: String,
    pub href: Option<String>,
    pub file: Option<FileRef>,
    pub content_type: Option<MediaType>,
    pub size: Option<u64>,
    pub rel: Option<String>,
    pub title: Option<String>,
}

/// Die Aufgabe (`JSCalendar` `Task`), vollständig typisiert. `labels` sind `JSCalendar` `keywords` (eine Menge).
#[derive(Clone, Debug, PartialEq)]
pub struct Task {
    pub id: ObjectId,
    pub list: ObjectId,
    pub uid: String,
    pub revision: Revision,
    pub sequence: u32,
    pub created: Utc,
    pub updated: Utc,
    pub title: String,
    pub description: String,
    pub description_type: TextType,
    pub locale: Option<String>,
    pub due: Option<Due>,
    pub start: Option<Due>,
    /// Geschätzter Aufwand (`estimatedDuration`), nicht die Länge eines Tages (ADR 0085).
    pub estimated: Option<Span>,
    pub progress: Progress,
    pub progress_updated: Option<Utc>,
    pub percent_complete: Option<u8>,
    /// 0 = nicht gesetzt, 1 = höchste … 9 = niedrigste (RFC 8984 `priority`).
    pub priority: u8,
    pub color: Option<String>,
    pub labels: Vec<String>,
    pub assignees: Vec<Assignee>,
    pub alerts: Vec<Alert>,
    pub links: Vec<Link>,
    /// Eltern-Aufgabe (Teilaufgabe; RFC 9253 `parent`/`child`). Kreise sind `TaskError::Cycle`; höchstens
    /// [`SUBTASK_DEPTH_MAX`] Ebenen.
    pub parent: Option<ObjectId>,
    pub dependencies: Vec<Dependency>,
    pub related: Vec<Related>,
    pub recurrence: Vec<Recurrence>,
    /// Zähl-Basis der Wiederholung (ohne `recurrence` bedeutungslos, `Schedule`).
    pub repeat_from: RepeatFrom,
    pub excluded_recurrence: Vec<Recurrence>,
    pub overrides: Vec<Override>,
    /// Checkliste, nach `rank` (siehe [`ChecklistItem`]); geändert nur über `Tasks::checklist`.
    pub checklist: Vec<ChecklistItem>,
    /// Handreihenfolge in Liste und Board (kleiner = weiter oben); gesetzt nur über `place` (Regeln: [`Rank`]).
    pub rank: Rank,
    pub claim: Option<Claim>,
    /// Herkunft, wenn importiert (nur lesend; `create`/`update` ignorieren es).
    pub origin: Option<Origin>,
}

/// Neue Aufgabe. `title` und `list` stehen NUR hier: `rest.title` und `rest.list` müssen `None` sein, sonst `Invalid`.
/// `uid = None`: der Anbieter vergibt eine. `progress`: Anfangsstand (Board "+ in dieser Spalte").
/// Die neue Aufgabe steht in ihrer Spalte oben (`rank`), bis `place` sie verschiebt. `assignees`: Personen oder
/// Gruppen. `checklist`: Titel der Anfangspunkte in dieser Reihenfolge, alle offen.
#[derive(Clone, Debug, PartialEq)]
pub struct TaskDraft {
    pub list: ObjectId,
    pub uid: Option<String>,
    pub title: String,
    pub progress: Progress,
    pub assignees: Vec<PersonRef>,
    pub checklist: Vec<String>,
    pub rest: TaskPatch,
}

/// Änderung: `Some` = Feld wird gesetzt (Listen ganz ersetzt, `labels` als Menge), `None` = bleibt. Stand, Zuweisung,
/// Reihenfolge, Checkliste und Abhängigkeiten haben eigene Methoden (eigene Regeln, eigenes Protokoll).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct TaskPatch {
    pub title: Option<String>,
    pub description: Option<(String, TextType)>,
    pub locale: Option<Option<String>>,
    pub due: Option<Option<Due>>,
    pub start: Option<Option<Due>>,
    pub estimated: Option<Option<Span>>,
    pub percent_complete: Option<Option<u8>>,
    pub priority: Option<u8>,
    pub color: Option<Option<String>>,
    pub labels: Option<Vec<String>>,
    pub alerts: Option<Vec<Alert>>,
    pub links: Option<Vec<Link>>,
    /// In eine andere Liste verschieben (nur `Whole`; Teilaufgaben ziehen mit).
    pub list: Option<ObjectId>,
    /// Unter eine andere Aufgabe hängen oder lösen (nur `Whole`).
    pub parent: Option<Option<ObjectId>>,
    pub recurrence: Option<Vec<Recurrence>>,
    /// Nur `Whole` (siehe [`RepeatFrom`]).
    pub repeat_from: Option<RepeatFrom>,
    pub excluded_recurrence: Option<Vec<Recurrence>>,
}

/// Ausnahme eines Vorkommens: fällt aus, oder weicht ab (Felder, Stand, Zugewiesene). In `patch` sind `list`, `parent`,
/// `recurrence`, `repeat_from` und `excluded_recurrence` nicht erlaubt (`Invalid`).
#[derive(Clone, Debug, PartialEq)]
// Selten und als Daten gedacht: die große Variante zu boxen würde die Form für jeden Anbieter ändern (`Changed(Box<..>)`).
#[allow(clippy::large_enum_variant)]
pub enum OverrideChange {
    Excluded,
    Changed {
        patch: TaskPatch,
        progress: Option<Progress>,
        assignees: Option<Vec<Assignee>>,
    },
}
/// Eine Ausnahme, benannt durch den ursprünglichen lokalen Bezugspunkt des Vorkommens.
#[derive(Clone, Debug, PartialEq)]
pub struct Override {
    pub recurrence_id: LocalDateTime,
    pub change: OverrideChange,
}

/// Höchste Tiefe des Teilaufgaben-Baums (die oberste Aufgabe ist Ebene 1; Regel in `mod.rs`, TEILAUFGABEN). Tiefer ist
/// `Invalid` (`parent`).
pub const SUBTASK_DEPTH_MAX: usize = 8;
