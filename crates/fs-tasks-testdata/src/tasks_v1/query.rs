//! Abfragen, Kurzform, Bereitschaft, Löschergebnis, Protokoll.
//!
//! Pattern: Value Object (Abfrage-, Antwort- und Protokolltypen). Ansichten (Liste, Board, Heute) sind Abfragen, keine
//! Daten.

use super::{
    AssigneeStatus, Date, Due, LocalDateTime, ObjectId, PersonRef, Progress, Rank, Reference,
    Revision, SeriesScope, Span, Utc, Zone,
};

/// Welche Fälligkeit ein Bereich meint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DateField {
    Due,
    Start,
    DueOrStart,
}
/// Tage `[from, to]` (beide eingeschlossen, `None` = offen) in der Zone des Betrachters: `Due::Date` und `Floating`
/// zählen mit ihrem Tag, `Timed` mit dem Tag in `view_zone`. "Heute" = `to: heute`, `from: None` (Überfälliges dabei).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DueRange {
    pub field: DateField,
    pub from: Option<Date>,
    pub to: Option<Date>,
}
/// Wem zugewiesen. `Me` = die fragende Person (`asker`); `Nobody` = keiner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssigneeFilter {
    Me,
    Person(PersonRef),
    Nobody,
}
/// Ebene: nur oberste Aufgaben, oder die Teilaufgaben einer Aufgabe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParentFilter {
    TopLevel,
    Of(ObjectId),
}
/// Sortierung. `Due`: undatierte zuletzt, gleiche Tage nach `rank`; `Rank`: Hand-Reihenfolge (Board, Liste);
/// `Finished`: zuletzt erledigt zuerst (Logbuch). Gleichstand immer nach `id` (stabile Seiten).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    Due,
    Rank,
    Priority,
    Updated,
    Finished,
}

/// Filter für `list`. Leere Listen = kein Filter. `view_zone` ist Pflicht (legt `Timed` auf einen Tag; eine eigene Zone
/// `ZoneKind::Custom` ergibt `UnknownZone`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskQuery {
    pub lists: Vec<ObjectId>,
    pub text: Option<String>,
    pub progress: Vec<Progress>,
    pub due: Option<DueRange>,
    /// `Some(true)`: nur ohne `due` und ohne `start`; `Some(false)`: nur mit einem von beiden.
    pub undated: Option<bool>,
    pub assignee: Option<AssigneeFilter>,
    /// Alle genannten Labels müssen gesetzt sein.
    pub labels: Vec<String>,
    pub parent: Option<ParentFilter>,
    /// `Some(true)`: nur `Readiness::Ready`; `Some(false)`: nur nicht bereit. Setzt `dependency.evaluate` voraus: ein
    /// Anbieter ohne diese Fähigkeit antwortet auf `Some(_)` mit `Unsupported { dependency.evaluate }` (nie "alle bereit").
    pub ready: Option<bool>,
    pub order: Order,
    pub view_zone: Zone,
    /// Papierkorb-Ansicht: ohne, gemischt oder nur Gelöschte (T20 listet mit `Only`, ohne Durchblättern).
    pub trash: Trash,
}

/// Papierkorb in einer Abfrage. Standard der Ansichten: `Exclude`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Trash {
    /// Nur lebende Aufgaben (bzw. Listen).
    #[default]
    Exclude,
    /// Lebende und gelöschte gemischt; `restorable_until` unterscheidet sie.
    Include,
    /// Nur gelöschte (die Papierkorb-Ansicht), nach `restorable_until` ihrer Frist.
    Only,
}

/// Ein Label mit der Zahl der lebenden Aufgaben, die es tragen (Filter, Vorschläge, Seitenleiste).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LabelCount {
    pub label: String,
    pub count: u32,
}

/// Was die Abhängigkeiten sagen (`uid`s, wie heute `plan::Readiness`; ADR 0073 Punkt 8: angezeigt, nie still verworfen).
///
/// Regel für Anbieter OHNE `dependency.evaluate` (z. B. `CalDAV`): eine Aufgabe ohne `dependencies` ist `Ready` bzw. `Done`
/// (nach `progress`); eine erledigte, abgebrochene oder gescheiterte ist immer `Done` (geht vor `NotEvaluated`); eine
/// offene MIT `dependencies` (auch aus einem VTODO mit `RELATED-TO`) ist `NotEvaluated`, nie still `Ready`.
/// `readiness` selbst und `TaskQuery::ready = Some(_)` sind dann `Unsupported`. Gewählt statt "Pflicht für alle", weil ein
/// Fremdanbieter die Abhängigkeiten nur speichern, aber nicht auswerten kann; die Abhängigkeiten (RFC 9253) bleiben in v1.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Readiness {
    Ready,
    /// Es gibt Abhängigkeiten, aber der Anbieter wertet sie nicht aus (siehe oben); ein Mensch sieht sie.
    NotEvaluated,
    /// Wartet auf diese Aufgaben; die Reihenfolge löst es.
    Blocked(Vec<String>),
    /// Eine Vorgängerin ist abgebrochen oder gescheitert: ein Mensch entscheidet.
    DependencyCancelled(Vec<String>),
    /// Selbst erledigt, abgebrochen oder gescheitert.
    Done,
    /// Eine Abhängigkeit nennt eine unbekannte `uid`: gezeigt, nie geraten.
    Unknown(Vec<String>),
    /// Teil eines Kreises (nach einem Abgleich entstanden), sortiert.
    Conflict(Vec<String>),
}

/// Kurzform für Liste, Board und "Heute". Serien erscheinen einmal mit ihrem aktuellen Vorkommen (`occurrence`: das
/// erste nicht erledigte, nicht ausgefallene; `due`/`progress`/`assignees` sind die dieses Vorkommens).
/// `restorable_until`: `None` = nicht gelöscht, `Some` = im Papierkorb bis dahin (nur mit `Trash::Include`/`Only`). Die
/// Kartenangaben (`has_description` .. `alerts`) genügen für Liste, Board-Karte und Zeile; mehr braucht `get`.
#[derive(Clone, Debug, PartialEq)]
pub struct TaskSummary {
    pub id: ObjectId,
    pub list: ObjectId,
    pub uid: String,
    pub title: String,
    pub due: Option<Due>,
    pub start: Option<Due>,
    pub occurrence: Option<LocalDateTime>,
    pub recurring: bool,
    pub progress: Progress,
    pub priority: u8,
    pub labels: Vec<String>,
    pub assignees: Vec<(PersonRef, AssigneeStatus)>,
    pub parent: Option<ObjectId>,
    pub subtasks_open: u32,
    pub subtasks_total: u32,
    pub readiness: Readiness,
    pub rank: Rank,
    pub claimed_by: Option<PersonRef>,
    pub revision: Revision,
    pub restorable_until: Option<Utc>,
    /// Notiz-Marke: die Beschreibung ist nicht leer.
    pub has_description: bool,
    pub percent_complete: Option<u8>,
    pub estimated: Option<Span>,
    /// Anzahl `links` und `alerts` (Büroklammer und Glocke auf der Karte).
    pub links: u32,
    pub alerts: u32,
}

/// Ergebnis eines Löschens (Papierkorb). Eine Aufgabe nimmt ihre Teilaufgaben mit (`with_subtasks`); `restore` mit
/// demselben `scope` holt alle zurück, bis die Frist endet.
///
/// `restorable_until = None`: der Anbieter hat keinen Papierkorb, das Löschen ist endgültig und `restore` ergibt
/// `NotFound`. Einen unbegrenzten Papierkorb gibt es nicht (jeder Papierkorb nennt eine Frist), damit `None` bei
/// `TaskSummary` ("lebt") eindeutig bleibt. Bei einer Liste (`delete_list`) ist `with_subtasks` die Menge aller Aufgaben der
/// Liste (auch Teilaufgaben) und `scope` immer `Whole`.
#[derive(Clone, Debug, PartialEq)]
pub struct Deleted {
    pub id: ObjectId,
    pub scope: SeriesScope,
    pub with_subtasks: Vec<ObjectId>,
    pub restorable_until: Option<Utc>,
}

/// Ein typisierter Eintrag im Aktivitätsprotokoll (ADR 0086 Punkt 2).
#[derive(Clone, Debug, PartialEq)]
pub struct HistoryEntry {
    pub at: Utc,
    pub actor: PersonRef,
    pub kind: HistoryKind,
    pub changes: Vec<FieldChange>,
}
/// Art der Aktivität (AS2-Name): `Add`/`Remove` = zuweisen/entfernen, `Accept`/`Reject` = Antwort der Person,
/// `Move` = Reihenfolge, Liste oder Eltern, `Join`/`Leave` = übernehmen/zurückgeben, `Undo` = wiederherstellen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryKind {
    Create,
    Update,
    Delete,
    Undo,
    Add,
    Remove,
    Accept,
    Reject,
    Move,
    Join,
    Leave,
}
/// Feld, alter Wert, neuer Wert (`row_key` = Schlüssel der Unterzeile, z. B. die Person einer Zuweisung).
#[derive(Clone, Debug, PartialEq)]
pub struct FieldChange {
    pub field: String,
    pub row_key: Option<String>,
    pub old: Option<Value>,
    pub new: Option<Value>,
}
/// Typisierter Wert im Protokoll. `Object` = eigene Objekt-Id, `Reference` = fremder Verweis. Aufzählungen (`progress`,
/// `due_kind` …) stehen als `Text` mit dem kanonischen `JSCalendar`- bzw. AS2-Wort.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Int(i64),
    Real(f64),
    Text(String),
    Bool(bool),
    Utc(Utc),
    Local(LocalDateTime),
    Date(Date),
    Zone(Zone),
    Object(ObjectId),
    Reference(Reference),
    Person(PersonRef),
}

/// Änderung für Beobachter (Liste und Board neu laden).
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    pub task: ObjectId,
    pub list: ObjectId,
    pub kind: HistoryKind,
    pub at: Utc,
}
