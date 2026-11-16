//! The tasks of the sample: Umzug and Website (projects), Alltag (today, series), Verein (waiting, groups).
//!
//! Pattern: Fixed data (one function, one line of builder calls per task). Cases covered: plain date, timed with
//! zone (Berlin, Tokyo), floating, undated; priorities 1 to 5; labels; assignees in every answer state; subtasks;
//! dependencies (ready, blocked, cancelled predecessor, unknown uid, a cycle); series (current occurrence, exceptions,
//! a rule outside the subset); the logbook (completed, cancelled, failed); a claim; a trashed task.

use crate::sample::{B, task};
use crate::tasks_v1::{
    AssigneeStatus::{Accepted, Declined, NeedsAction as Open},
    DependencyKind::{DependsOn, FinishToStart},
    Progress, Task,
};

type Pair = (&'static str, Task);

fn push(v: &mut Vec<Pair>, b: B) {
    v.push((b.slug, b.t));
}

/// All tasks (list 1 to 4) and the one in the trash.
pub(crate) fn tasks() -> (Vec<Pair>, Pair) {
    let mut v = Vec::new();
    umzug(&mut v);
    website(&mut v);
    crate::sample_more::alltag(&mut v);
    crate::sample_more::verein(&mut v);
    crate::sample_extra::more(&mut v);
    crate::sample_delta::delta(&mut v);
    let gone = task(399, 3, "altes-abo", "Altes Zeitschriften-Abo kündigen")
        .due("2026-11-30")
        .labels(&["geld"]);
    (v, (gone.slug, gone.t))
}

fn umzug(v: &mut Vec<Pair>) {
    push(
        v,
        task(101, 1, "kartons", "Kartons besorgen")
            .due("2026-11-13")
            .prio(2)
            .labels(&["umzug", "einkauf"])
            .who(&[(1, Accepted)]),
    );
    push(
        v,
        task(102, 1, "mietvertrag", "Mietvertrag unterschreiben")
            .labels(&["papier"])
            .state(Progress::Completed, 6),
    );
    push(
        v,
        task(103, 1, "nachsendeauftrag", "Nachsendeauftrag einrichten")
            .due("2026-11-16")
            .prio(1)
            .labels(&["papier"])
            .who(&[(1, Accepted)])
            .desc("Online beantragen, mindestens fünf Werktage vor dem Umzug."),
    );
    push(
        v,
        task(104, 1, "transporter", "Transporter mieten")
            .due_at("2026-11-20T09:00:00", "Europe/Berlin")
            .prio(3)
            .est(0, 14_400)
            .who(&[(2, Accepted), (3, Open)]),
    );
    push(
        v,
        task(105, 1, "einpacken", "Alles einpacken")
            .due("2026-11-21")
            .prio(2)
            .labels(&["umzug"])
            .pct(33),
    );
    push(
        v,
        task(106, 1, "kueche", "Küche")
            .parent(105)
            .state(Progress::Completed, 2),
    );
    push(
        v,
        task(107, 1, "buecher", "Bücher")
            .parent(105)
            .who(&[(3, Open)]),
    );
    push(
        v,
        task(108, 1, "keller", "Keller ausräumen")
            .parent(105)
            .due("2026-11-19")
            .who(&[(4, Declined)]),
    );
    push(
        v,
        task(109, 1, "strom", "Strom ummelden")
            .dep("mietvertrag", DependsOn)
            .due("2026-11-18")
            .labels(&["papier"]),
    );
    push(
        v,
        task(110, 1, "zaehlerstand", "Zählerstand fotografieren").dep("transporter", DependsOn),
    );
    push(
        v,
        task(111, 1, "sofa", "Altes Sofa verschenken").dep("fremd-unbekannt", DependsOn),
    );
    push(
        v,
        task(112, 1, "schluessel", "Schlüsselübergabe vereinbaren").dep("besichtigung", DependsOn),
    );
    push(
        v,
        task(113, 1, "besichtigung", "Besichtigungstermin abstimmen").state(Progress::Cancelled, 9),
    );
}

fn website(v: &mut Vec<Pair>) {
    push(
        v,
        task(201, 2, "startseite", "Entwurf Startseite")
            .state(Progress::InProcess, 0)
            .prio(2)
            .labels(&["design"])
            .who(&[(3, Accepted)])
            .pct(60)
            .est(2, 0),
    );
    push(
        v,
        task(202, 2, "ueber-uns", "Texte für Über uns")
            .prio(3)
            .labels(&["text"])
            .who(&[(4, Open)])
            .due("2026-11-24"),
    );
    push(
        v,
        task(203, 2, "navigation", "Navigation testen")
            .dep("startseite", DependsOn)
            .labels(&["test"])
            .prio(4),
    );
    push(
        v,
        task(204, 2, "kontakt", "Kontaktformular bauen")
            .labels(&["text"])
            .dep("bilder", FinishToStart),
    );
    push(
        v,
        task(205, 2, "bilder", "Bilder einbauen").labels(&["design"]),
    );
    push(
        v,
        task(206, 2, "domain", "Domain prüfen").state(Progress::Completed, 7),
    );
    push(
        v,
        task(207, 2, "hosting", "Hosting vergleichen")
            .state(Progress::Completed, 4)
            .labels(&["recherche"]),
    );
    push(
        v,
        task(208, 2, "logo", "Logo-Varianten sammeln")
            .state(Progress::Completed, 2)
            .who(&[(3, Accepted)]),
    );
    push(
        v,
        task(209, 2, "farben", "Farbpalette festlegen").state(Progress::Cancelled, 5),
    );
    push(
        v,
        task(210, 2, "foto", "Foto-Termin organisieren")
            .state(Progress::Failed, 3)
            .desc("Fotografin war nicht verfügbar."),
    );
    push(
        v,
        task(211, 2, "checkliste", "Release-Checkliste").dep("freigabe", DependsOn),
    );
    push(
        v,
        task(212, 2, "freigabe", "Freigabe einholen").dep("checkliste", DependsOn),
    );
    push(
        v,
        task(213, 2, "barrierefrei", "Barrierefreiheit prüfen")
            .floating("2026-11-18T12:00:00")
            .prio(4)
            .pct(40)
            .state(Progress::InProcess, 0)
            .labels(&["test", "a11y"])
            .who(&[(2, Accepted), (3, Open)]),
    );
    push(
        v,
        task(214, 2, "partnerbuero", "Abstimmung mit Partnerbüro")
            .due_at("2026-11-17T08:00:00", "Asia/Tokyo")
            .prio(2)
            .labels(&["test"])
            .desc("08:00 in Tokio ist in Berlin schon der 17. um 00:00."),
    );
}
