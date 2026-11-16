//! Der Trait `Tasks` und seine Fähigkeitskennungen.
//!
//! Pattern: Repository (Trait) + Port: jede Methode ist Pflicht, kein Standardverhalten; was ein Anbieter nicht kann,
//! sagt er vorher in `capabilities` und beim Aufruf ausdrücklich mit [`super::not_possible`] (`Unsupported`).

use super::{
    Answer, Capabilities, Change, ChecklistChange, Children, Deleted, Dependency, DependencyKind,
    HistoryEntry, LabelCount, ObjectId, Page, PersonRef, Place, Progress, Readiness, Revision,
    SeriesScope, Task, TaskDraft, TaskList, TaskListPatch, TaskPatch, TaskQuery, TaskResult,
    TaskSummary, Trash, Utc,
};
use async_trait::async_trait;

/// Fähigkeitskennungen dieses Themas (Wortlaut für `capabilities`, Manifest und `tasks.capnp` v1). Bisherige Kennungen
/// bleiben gleich (`task.create`, `task.edit`, `task.assign`, `relation.create`, `task.claim`, `task.delete`,
/// `dependency.evaluate`); `label.assign` geht in `task.edit` auf. `task.read` deckt `capabilities`, `asker`, `get`,
/// `get_by_uid`, `list`; jede andere Methode hat ihre Kennung (geteilt: `list.delete` = `delete_list` +
/// `restore_list`, `task.assign` = `assign` + `unassign`, `task.claim` = `claim` + `release` + `claim_next`).
/// `true` heißt: sie antwortet nie `Unsupported`. `relation.create` setzt `dependency.evaluate` nicht voraus.
pub const CAPABILITIES: [&str; 22] = [
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
    "task.checklist",
    "relation.create",
    "relation.delete",
    "dependency.evaluate",
    "task.claim",
    "task.delete",
    "task.restore",
    "trash.purge",
    "task.history",
];

/// Aufgaben: Listen, anlegen, ändern, zuweisen, erledigen, ordnen, Checklisten, Abhängigkeiten, Übernahme,
/// Papierkorb, Protokoll.
///
/// Ziel v1 (ADR 0045). Erster Anbieter: lokal über fs-objects v5 (`LocalTasks`); `RemoteTasks` über `tasks.capnp` v1;
/// später `CalDAV` (VTODO, gemeinsamer DAV-Wrapper). Regeln des Vertrags (Vergleich-und-Setzen, fragende Person,
/// Eigentum, claim-Regel, Papierkorb, Teilaufgaben-Baum): `mod.rs`.
#[async_trait]
pub trait Tasks: Send + Sync {
    /// Was dieser Anbieter kann, je Kennung aus [`CAPABILITIES`]. Das Frontend zeigt nur, was `true` ist.
    fn capabilities(&self) -> Capabilities;
    /// `task.read`: die fragende Person ("mir zuweisen", `AssigneeFilter::Me`). Ohne Ich-Karte (bei
    /// `PersonDirectory::asker` `None`) der Konto-Verweis des Kontos, nie ein Fehler.
    async fn asker(&self) -> TaskResult<PersonRef>;

    /// `list.read`: alle Aufgabenlisten, die die fragende Person sehen darf (eigene und die ihrer Gruppen), je mit
    /// [`TaskList::access`]; `trash` wie bei Aufgaben.
    async fn lists(&self, trash: Trash) -> TaskResult<Vec<TaskList>>;
    /// `list.create`: `owner` `None` = die fragende Person; `Some` = eine Gruppe/Organisation (die fragende Person
    /// braucht dort das Recht dazu, sonst `Forbidden`), eine andere Person ist `Invalid`, eine KI `Forbidden`.
    async fn create_list(
        &self,
        name: &str,
        color: Option<&str>,
        owner: Option<&PersonRef>,
    ) -> TaskResult<TaskList>;
    /// `list.edit`: umbenennen, umfärben, Eigentum übergeben ([`TaskListPatch`]); eine gelöschte Liste ist `NotFound`.
    async fn update_list(
        &self,
        id: ObjectId,
        expected: Revision,
        change: TaskListPatch,
    ) -> TaskResult<TaskList>;
    /// `list.delete`: Liste in den Papierkorb, MIT allen ihren Aufgaben (auch Teilaufgaben, `Deleted::with_subtasks`),
    /// alle mit derselben Frist. Die Aufgaben sind einzeln nicht wiederherstellbar (`restore` ergibt `Invalid`,
    /// solange ihre Liste im Papierkorb liegt) und neue Aufgaben darin sind `NotFound`. `restore_list` holt die Liste
    /// mitsamt den Aufgaben zurück, die beim Löschen darin lagen; vorher einzeln gelöschte bleiben, wo sie sind. Geht
    /// die Liste endgültig (`purge`, Ablauf ihrer Frist), gehen die vorher einzeln gelöschten Aufgaben mit (siehe
    /// `purge`): ohne Liste leben sie nicht.
    async fn delete_list(&self, id: ObjectId, expected: Revision) -> TaskResult<Deleted>;
    /// `list.delete`: Liste und ihre mitgelöschten Aufgaben aus dem Papierkorb holen, solange die Frist gilt.
    async fn restore_list(&self, id: ObjectId) -> TaskResult<TaskList>;

    /// `task.read`; eine gelöschte Aufgabe ergibt `None` (sichtbar nur über `list` mit `Trash::Include`/`Only`).
    async fn get(&self, id: ObjectId) -> TaskResult<Option<Task>>;
    /// `task.read` über die `uid`.
    async fn get_by_uid(&self, uid: &str) -> TaskResult<Option<Task>>;
    /// `task.read`: Liste, Suche, "Heute", Board-Spalte, Teilaufgaben, Team-Eingang; seitenweise.
    async fn list(
        &self,
        query: &TaskQuery,
        after: Option<&Place>,
        size: usize,
    ) -> TaskResult<Page<TaskSummary>>;
    /// `task.labels`: die Labels der lebenden Aufgaben in `lists` (leer = alle sichtbaren Listen) mit Zahl, die mit
    /// `prefix` beginnen (`None` = alle), alphabetisch, seitenweise. Für Filter (T12) und Vorschläge (T13).
    async fn labels(
        &self,
        lists: &[ObjectId],
        prefix: Option<&str>,
        after: Option<&Place>,
        size: usize,
    ) -> TaskResult<Page<LabelCount>>;
    /// `dependency.evaluate`: je Id ein Wert, in der gefragten Reihenfolge; eine unbekannte Id ergibt `NotFound` für den
    /// ganzen Aufruf (nichts fällt still weg).
    async fn readiness(&self, ids: &[ObjectId]) -> TaskResult<Vec<(ObjectId, Readiness)>>;

    /// `task.create`: prüfen und speichern. Abgelehnt, wenn die `uid` schon existiert.
    async fn create(&self, draft: TaskDraft) -> TaskResult<Task>;
    /// `task.edit`: Felder ändern; `scope` bei Serien (sonst `Whole`).
    async fn update(
        &self,
        id: ObjectId,
        expected: Revision,
        change: TaskPatch,
        scope: SeriesScope,
    ) -> TaskResult<Task>;
    /// `task.assign`: `who` (Person oder Gruppe) zuweisen (Status `NeedsAction`, `assigned_by` = fragende Person);
    /// schon zugewiesen = keine Änderung, keine neue Revision.
    async fn assign(
        &self,
        id: ObjectId,
        expected: Revision,
        who: &PersonRef,
        scope: SeriesScope,
    ) -> TaskResult<Task>;
    /// `task.assign`: Zuweisung von `who` entfernen; nicht zugewiesen = keine Änderung, keine neue Revision. Eine
    /// Übernahme, die nur über `who` erlaubt war, bleibt bis zu ihrem Ende bestehen.
    async fn unassign(
        &self,
        id: ObjectId,
        expected: Revision,
        who: &PersonRef,
        scope: SeriesScope,
    ) -> TaskResult<Task>;
    /// `task.respond`: die fragende Person nimmt IHRE Zuweisung an oder weist sie ab (schreibt nur ihre Zeile). Nicht
    /// selbst zugewiesen (auch: nur über eine Gruppe) = `Forbidden`.
    async fn respond(
        &self,
        id: ObjectId,
        expected: Revision,
        answer: Answer,
        comment: Option<&str>,
        scope: SeriesScope,
    ) -> TaskResult<Task>;
    /// `task.progress`: erledigen, wieder öffnen, in Arbeit, abbrechen, gescheitert; auch der Spaltenwechsel im Board.
    /// Bei einer Serie nach Plan mit `This` wird nur dieses Vorkommen erledigt, das nächste wird das aktuelle (Things
    /// T7); ab Erledigung rückt dieselbe Aufgabe weiter ([`super::RepeatFrom::Completion`]). `children` regelt
    /// offene Teilaufgaben (Checklisten-Punkte bleiben, wie sie sind).
    async fn progress(
        &self,
        id: ObjectId,
        expected: Revision,
        to: Progress,
        children: Children,
        scope: SeriesScope,
    ) -> TaskResult<Task>;
    /// `task.place`: Hand-Reihenfolge; direkt nach `after` einsortieren (`None` = ganz oben). Bezug ist die Rangfolge
    /// (dieselbe Liste, Eltern-Aufgabe, `progress`); ein `after` außerhalb davon ist `Invalid`. Schreibt nur diese
    /// Aufgabe (Ausnahme: einmaliges Neu-Verteilen, siehe [`super::Rank`]). Ziehen im Board = erst `progress`, dann
    /// `place` mit der neuen Revision; scheitert `place` mit `Changed`, wiederholt das Frontend `place`.
    async fn place(
        &self,
        id: ObjectId,
        expected: Revision,
        after: Option<ObjectId>,
    ) -> TaskResult<Task>;
    /// `task.checklist`: einen Punkt anlegen, umbenennen, abhaken, verschieben, entfernen (siehe
    /// [`super::ChecklistItem`]); immer die ganze Serie, das Abhaken gilt dem aktuellen Vorkommen.
    async fn checklist(
        &self,
        id: ObjectId,
        expected: Revision,
        change: ChecklistChange,
    ) -> TaskResult<Task>;

    /// `relation.create`: Abhängigkeit zu `other_uid`; ein Kreis ist `TaskError::Cycle`; `gap` bei `DependsOn` ist
    /// `Invalid`.
    async fn relate(
        &self,
        id: ObjectId,
        expected: Revision,
        dependency: Dependency,
    ) -> TaskResult<Task>;
    /// `relation.delete`: Abhängigkeit zu `other_uid` der Art `kind` lösen; nicht vorhanden = `Invalid`.
    async fn unrelate(
        &self,
        id: ObjectId,
        expected: Revision,
        other_uid: &str,
        kind: DependencyKind,
    ) -> TaskResult<Task>;

    /// `task.claim`: für die fragende Person bis `until` übernehmen (claim-Regel in `mod.rs`: nicht berechtigt =
    /// `Forbidden`); hält eine andere sie noch: `TaskError::Claimed`. Die eigene Übernahme darf erneuert werden. Eine
    /// erledigte, abgebrochene oder gescheiterte Aufgabe ist `Invalid`. Reihenfolge der Prüfung (gilt auch für
    /// `release`): nicht lesbar oder unbekannt `NotFound`, dann claim-Regel `Forbidden`, dann Haltende `Claimed`; eine
    /// abgelaufene Übernahme (`until` vorbei) hält niemand mehr.
    async fn claim(&self, id: ObjectId, expected: Revision, until: Utc) -> TaskResult<Task>;
    /// `task.claim`: zurückgeben; nur wer sie hält, eine abgelaufene darf jeder, sonst `TaskError::Claimed`.
    async fn release(&self, id: ObjectId, expected: Revision) -> TaskResult<Task>;
    /// `task.claim`: "die nächste für mich": die erste Aufgabe in der Reihenfolge von `query`, die offen
    /// (`NeedsAction`/`InProcess`), nicht von anderen gehalten (abgelaufene Übernahmen gelten als frei) und für die
    /// fragende Person nach der claim-Regel
    /// übernehmbar ist, in EINER Transaktion übernehmen (kein Wettlauf zwischen `list` und `claim`). Mit
    /// `dependency.evaluate` nur `Readiness::Ready`; ohne werden Aufgaben mit Abhängigkeiten übersprungen (nie eine
    /// womöglich blockierte). `None` = keine passende. `query.ready` gesetzt oder `trash` nicht `Exclude`: `Invalid`.
    async fn claim_next(&self, query: &TaskQuery, until: Utc) -> TaskResult<Option<Task>>;

    /// `task.delete`: in den Papierkorb, mit den lebenden Teilaufgaben; `This` legt eine Ausnahme `Excluded` an,
    /// `ThisAndFollowing` kürzt die Serie (`RuleEnd::Until`).
    async fn delete(
        &self,
        id: ObjectId,
        expected: Revision,
        scope: SeriesScope,
    ) -> TaskResult<Deleted>;
    /// `task.restore`: Löschen rückgängig, solange `restorable` gilt, mit demselben `scope` wie beim Löschen; die
    /// mitgelöschten Teilaufgaben kommen mit. Liegt der Elternteil oder die Liste im Papierkorb: `Invalid`.
    async fn restore(&self, id: ObjectId, scope: SeriesScope) -> TaskResult<Task>;
    /// `trash.purge`: eine Aufgabe (samt mitgelöschten Teilaufgaben) oder eine Liste (samt mitgelöschten Aufgaben) im
    /// Papierkorb jetzt endgültig löschen, mit Verlauf und Checklisten (ADR 0073 "wirklich löschen"; der Weg bei
    /// `Restorable::Manual`). Nicht im Papierkorb: `Invalid`; aufbewahrt (`Held`): `Forbidden { trash.purge }`, nichts
    /// wird gelöscht. LISTE (wie `appointments_v1`): `purge` und der Ablauf ihrer Frist nehmen auch die vorher einzeln
    /// gelöschten Aufgaben der Liste mit (ohne Liste leben sie nicht; ihre eigene Frist endet damit). Liegt darin eine
    /// aufbewahrte Aufgabe, ist `purge` der Liste `Forbidden { trash.purge }`, und die Liste bleibt im Papierkorb
    /// (auch nach Ablauf ihrer Frist), bis die Aufbewahrung endet.
    async fn purge(&self, id: ObjectId) -> TaskResult<()>;

    /// `task.history`: typisiertes Protokoll einer Aufgabe, älteste zuerst.
    async fn history(
        &self,
        id: ObjectId,
        after: Option<&Place>,
        size: usize,
    ) -> TaskResult<Page<HistoryEntry>>;
    /// `task.changes`: Änderungen seit `since` (`None` = ab jetzt), für Beobachter (Liste und Board neu laden). Wer es
    /// nicht kann, meldet `task.changes = false`, und das Frontend pollt per `list`. `Page::next` ist IMMER gesetzt,
    /// auch bei leerer Seite.
    async fn changes(&self, since: Option<&Place>, size: usize) -> TaskResult<Page<Change>>;
}
