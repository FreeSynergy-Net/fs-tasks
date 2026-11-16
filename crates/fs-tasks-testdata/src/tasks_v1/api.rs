//! Der Trait `Tasks` und seine Fähigkeitskennungen.
//!
//! Pattern: Repository (Trait) mit Null Object (nicht überschriebene Schreibmethoden sind `Unsupported`).

use super::{
    not_possible, Answer, Capabilities, Change, Children, Deleted, Dependency, DependencyKind,
    HistoryEntry, LabelCount, ObjectId, Page, PersonRef, Place, Progress, Readiness, Revision,
    SeriesScope, Task, TaskDraft, TaskList, TaskPatch, TaskQuery, TaskResult, TaskSummary, Trash,
    Utc,
};
use async_trait::async_trait;

/// Fähigkeitskennungen dieses Themas (Wortlaut für `capabilities`, Manifest und `tasks.capnp` v1). Bisherige Kennungen
/// bleiben gleich (`task.create`, `task.edit`, `task.assign`, `relation.create`, `task.claim`, `task.delete`,
/// `dependency.evaluate`); `label.assign` geht in `task.edit` auf. Jede Methode mit Standardantwort `Unsupported` hat ihre
/// EIGENE Kennung (`task.read` deckt nur `asker`, `get`, `get_by_uid`, `list`): `true` heißt, sie antwortet nie `Unsupported`.
/// `relation.create` setzt `dependency.evaluate` nicht voraus (siehe [`Readiness`]).
pub const CAPABILITIES: [&str; 20] = [
    "list.read",
    "list.create",
    "list.edit",
    "list.delete",
    "task.read",
    "task.labels",
    "task.changes",
    "task.create",
    "task.edit",
    "task.assign",
    "task.respond",
    "task.progress",
    "task.place",
    "relation.create",
    "relation.delete",
    "dependency.evaluate",
    "task.claim",
    "task.delete",
    "task.restore",
    "task.history",
];

/// Aufgaben: Listen, anlegen, ändern, zuweisen, erledigen, ordnen, Abhängigkeiten, Papierkorb, Protokoll.
///
/// Ziel v1 (ADR 0045). Erster Anbieter: lokal über fs-objects v5 (`LocalTasks`); `RemoteTasks` über `tasks.capnp` v1;
/// später `CalDAV` (VTODO). Schreiben ist immer Vergleich-und-Setzen gegen `expected` (außer `create`, `create_list` und
/// `restore`). Wer handelt, ist die fragende Person des Anbieters (`asker`). Ein fallengelassenes Future bricht nichts
/// ab, was schon geschrieben wird (eine Transaktion je Aufruf).
#[async_trait]
pub trait Tasks: Send + Sync {
    /// Was dieser Anbieter kann, je Kennung aus [`CAPABILITIES`]. Das Frontend zeigt nur, was `true` ist.
    fn capabilities(&self) -> Capabilities;
    /// `task.read`: die fragende Person ("mir zuweisen", Filter `AssigneeFilter::Me`). Hat sie keine Ich-Karte (bei
    /// `PersonDirectory::asker` wäre das `None`), liefert der Anbieter den Konto-Verweis (`PersonRef`) des Kontos, nie einen
    /// Fehler (sonst bricht "Mir zuweisen" beim ersten Start).
    async fn asker(&self) -> TaskResult<PersonRef>;

    /// `list.read`: alle Aufgabenlisten, die die fragende Person sehen darf; `trash` wie bei Aufgaben (Standard
    /// `Trash::Exclude`).
    async fn lists(&self, trash: Trash) -> TaskResult<Vec<TaskList>>;
    /// `list.create`
    async fn create_list(&self, _name: &str, _color: Option<&str>) -> TaskResult<TaskList> {
        not_possible("list.create")
    }
    /// `list.edit`: umbenennen (`name`) und umfärben (`color`: `Some(None)` = Farbe entfernen); Vergleich-und-Setzen gegen
    /// `expected`. Ein leerer Name ist `Invalid`; eine gelöschte Liste ist `NotFound`.
    async fn update_list(
        &self,
        _id: ObjectId,
        _expected: Revision,
        _name: Option<&str>,
        _color: Option<Option<&str>>,
    ) -> TaskResult<TaskList> {
        not_possible("list.edit")
    }
    /// `list.delete`: Liste in den Papierkorb, MIT allen ihren Aufgaben (auch Teilaufgaben, `Deleted::with_subtasks`),
    /// alle mit derselben Frist `restorable_until`. Die Aufgaben sind einzeln nicht wiederherstellbar (`restore` ergibt
    /// `Invalid`, solange ihre Liste im Papierkorb liegt) und neue Aufgaben in der Liste sind `NotFound`. Die Liste
    /// kommt mit `restore_list` zurück, mitsamt den Aufgaben, die beim Löschen darin lagen; später verschobene oder einzeln
    /// gelöschte Aufgaben bleiben, wo sie sind. Nach der Frist ist alles endgültig weg.
    async fn delete_list(&self, _id: ObjectId, _expected: Revision) -> TaskResult<Deleted> {
        not_possible("list.delete")
    }
    /// `list.delete`: Liste und ihre mitgelöschten Aufgaben aus dem Papierkorb holen, solange die Frist nicht vorbei ist.
    async fn restore_list(&self, _id: ObjectId) -> TaskResult<TaskList> {
        not_possible("list.delete")
    }

    /// `task.read`; eine gelöschte Aufgabe ergibt `None` (sichtbar nur über `list` mit `Trash::Include`/`Only`).
    async fn get(&self, id: ObjectId) -> TaskResult<Option<Task>>;
    /// `task.read` über die `uid`.
    async fn get_by_uid(&self, uid: &str) -> TaskResult<Option<Task>>;
    /// `task.read`: Liste, Suche (`text` in Titel und Beschreibung), "Heute", Board-Spalte, Teilaufgaben; seitenweise.
    async fn list(
        &self,
        query: &TaskQuery,
        after: Option<&Place>,
        size: usize,
    ) -> TaskResult<Page<TaskSummary>>;
    /// `task.labels`: die vorhandenen Labels der lebenden Aufgaben in `lists` (leer = alle sichtbaren Listen) mit Zahl,
    /// die mit `prefix` beginnen (`None` = alle), alphabetisch, seitenweise. Für Filter (T12) und Vorschläge (T13).
    async fn labels(
        &self,
        _lists: &[ObjectId],
        _prefix: Option<&str>,
        _after: Option<&Place>,
        _size: usize,
    ) -> TaskResult<Page<LabelCount>> {
        not_possible("task.labels")
    }
    /// `dependency.evaluate`: je Id ein Wert, in der gefragten Reihenfolge; eine unbekannte Id ergibt `NotFound` für den
    /// ganzen Aufruf (nichts fällt still weg).
    async fn readiness(&self, _ids: &[ObjectId]) -> TaskResult<Vec<(ObjectId, Readiness)>> {
        not_possible("dependency.evaluate")
    }

    /// `task.create`: prüfen und speichern. Abgelehnt, wenn die `uid` schon existiert.
    async fn create(&self, _draft: TaskDraft) -> TaskResult<Task> {
        not_possible("task.create")
    }
    /// `task.edit`: Felder ändern; `scope` bei Serien (sonst `Whole`).
    async fn update(
        &self,
        _id: ObjectId,
        _expected: Revision,
        _change: TaskPatch,
        _scope: SeriesScope,
    ) -> TaskResult<Task> {
        not_possible("task.edit")
    }
    /// `task.assign`: `who` zuweisen (Status `NeedsAction`, `assigned_by` = fragende Person); schon zugewiesen = keine
    /// Änderung, keine neue Revision.
    async fn assign(
        &self,
        _id: ObjectId,
        _expected: Revision,
        _who: &PersonRef,
        _scope: SeriesScope,
    ) -> TaskResult<Task> {
        not_possible("task.assign")
    }
    /// `task.assign`: Zuweisung von `who` entfernen; nicht zugewiesen = keine Änderung, keine neue Revision (wie `assign`,
    /// beide idempotent).
    async fn unassign(
        &self,
        _id: ObjectId,
        _expected: Revision,
        _who: &PersonRef,
        _scope: SeriesScope,
    ) -> TaskResult<Task> {
        not_possible("task.assign")
    }
    /// `task.respond`: die fragende Person nimmt ihre Zuweisung an oder weist sie ab (schreibt nur ihre Zeile). Nicht
    /// zugewiesen = `Forbidden`.
    async fn respond(
        &self,
        _id: ObjectId,
        _expected: Revision,
        _answer: Answer,
        _comment: Option<&str>,
        _scope: SeriesScope,
    ) -> TaskResult<Task> {
        not_possible("task.respond")
    }
    /// `task.progress`: erledigen (`Completed`), wieder öffnen (`NeedsAction`), in Arbeit, abbrechen, gescheitert; das ist
    /// auch der Spaltenwechsel im Board. Bei einer Serie mit `This` wird nur dieses Vorkommen erledigt, das nächste wird
    /// das aktuelle (Things T7). `children` regelt offene Teilaufgaben.
    async fn progress(
        &self,
        _id: ObjectId,
        _expected: Revision,
        _to: Progress,
        _children: Children,
        _scope: SeriesScope,
    ) -> TaskResult<Task> {
        not_possible("task.progress")
    }
    /// `task.place`: Hand-Reihenfolge; direkt nach `after` einsortieren (`None` = ganz oben). Bezug ist die Rangfolge der
    /// Aufgabe: dieselbe Liste, dieselbe Eltern-Aufgabe, derselbe `progress` (die Board-Spalte); ein `after` außerhalb
    /// davon ist `Invalid`. Schreibt nur diese Aufgabe, siehe [`Rank`](super::Rank): kein Neu-Nummerieren; einzige
    /// Ausnahme ist das einmalige Neu-Verteilen der Rangfolge bei `Exhausted`/`Equal` (gleiche Transaktion, auch bei
    /// `create` und `progress`). Ziehen im Board =
    /// zwei Transaktionen, erst `progress`, dann `place` mit der neuen Revision; scheitert `place` mit `Changed`, steht die
    /// Karte oben in der neuen Spalte und das Frontend wiederholt `place` mit der neuen Revision.
    async fn place(
        &self,
        _id: ObjectId,
        _expected: Revision,
        _after: Option<ObjectId>,
    ) -> TaskResult<Task> {
        not_possible("task.place")
    }

    /// `relation.create`: Abhängigkeit zu `other_uid`; ein Kreis ist `TaskError::Cycle`; `gap` bei `DependsOn` ist
    /// `Invalid`.
    async fn relate(
        &self,
        _id: ObjectId,
        _expected: Revision,
        _dependency: Dependency,
    ) -> TaskResult<Task> {
        not_possible("relation.create")
    }
    /// `relation.delete`: Abhängigkeit zu `other_uid` der Art `kind` lösen; nicht vorhanden = `Invalid`.
    async fn unrelate(
        &self,
        _id: ObjectId,
        _expected: Revision,
        _other_uid: &str,
        _kind: DependencyKind,
    ) -> TaskResult<Task> {
        not_possible("relation.delete")
    }

    /// `task.claim`: für die fragende Person bis `until` übernehmen; hält eine andere sie noch: `TaskError::Claimed`.
    /// Die eigene Übernahme darf erneuert werden.
    async fn claim(&self, _id: ObjectId, _expected: Revision, _until: Utc) -> TaskResult<Task> {
        not_possible("task.claim")
    }
    /// `task.claim`: zurückgeben; nur wer sie hält (eine abgelaufene darf jeder): sonst `TaskError::Claimed`.
    async fn release(&self, _id: ObjectId, _expected: Revision) -> TaskResult<Task> {
        not_possible("task.claim")
    }

    /// `task.delete`: in den Papierkorb, mit den Teilaufgaben; `This` legt eine Ausnahme `Excluded` an,
    /// `ThisAndFollowing` kürzt die Serie (`RuleEnd::Until`).
    async fn delete(
        &self,
        _id: ObjectId,
        _expected: Revision,
        _scope: SeriesScope,
    ) -> TaskResult<Deleted> {
        not_possible("task.delete")
    }
    /// `task.restore`: Löschen rückgängig, solange `restorable_until` nicht vorbei ist, mit demselben `scope` wie beim
    /// Löschen; die mitgelöschten Teilaufgaben kommen mit.
    async fn restore(&self, _id: ObjectId, _scope: SeriesScope) -> TaskResult<Task> {
        not_possible("task.restore")
    }

    /// `task.history`: typisiertes Protokoll einer Aufgabe, älteste zuerst.
    async fn history(
        &self,
        _id: ObjectId,
        _after: Option<&Place>,
        _size: usize,
    ) -> TaskResult<Page<HistoryEntry>> {
        not_possible("task.history")
    }
    /// `task.changes`: Änderungen seit `since` (`None` = ab jetzt), für Beobachter (Liste und Board neu laden). Keine
    /// Standardantwort: wer es nicht kann, meldet `task.changes = false` und das Frontend fragt gar nicht erst (pollt
    /// dann per `list`). `Page::next` ist IMMER gesetzt, auch bei leerer Seite (sonst kann "ab jetzt" nicht weiterfragen).
    async fn changes(&self, since: Option<&Place>, size: usize) -> TaskResult<Page<Change>>;
}
