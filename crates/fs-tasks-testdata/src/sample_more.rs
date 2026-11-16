//! More tasks of the sample: Alltag (today, series, subtasks) and Verein (waiting, groups, a claim).
//!
//! Pattern: Fixed data (one function, one line of builder calls per task); the cases are listed in `sample_data.rs`.

use crate::ids::person;
use crate::sample::{B, task};
use crate::tasks_v1::{
    AssigneeStatus::{Accepted, NeedsAction as Open},
    Claim,
    DependencyKind::DependsOn,
    Frequency, Progress, RecurrenceRule, RuleEnd, Task, Utc,
};

type Pair = (&'static str, Task);

fn push(v: &mut Vec<Pair>, b: B) {
    v.push((b.slug, b.t));
}

fn every(f: Frequency, interval: u32, end: RuleEnd) -> RecurrenceRule {
    RecurrenceRule {
        end,
        ..RecurrenceRule::every(f, interval)
    }
}

pub(crate) fn alltag(v: &mut Vec<Pair>) {
    push(
        v,
        task(301, 3, "biotonne", "Biotonne rausstellen")
            .due("2026-11-16")
            .series(every(Frequency::Weekly, 2, RuleEnd::Never))
            .labels(&["haus"]),
    );
    push(
        v,
        task(302, 3, "pflanzen", "Pflanzen gießen")
            .due("2026-11-16")
            .series(every(Frequency::Daily, 1, RuleEnd::Count(14)))
            .done_on("2026-11-16T00:00:00")
            .skip_on("2026-11-18T00:00:00"),
    );
    push(
        v,
        task(303, 3, "steuer", "Steuererklärung abgeben")
            .due("2027-05-31")
            .series(every(Frequency::Yearly, 1, RuleEnd::Never))
            .labels(&["geld", "papier"]),
    );
    push(
        v,
        task(304, 3, "zahnarzt", "Zahnarzt anrufen")
            .due_at("2026-11-16T15:00:00", "Europe/Berlin")
            .prio(1),
    );
    push(
        v,
        task(305, 3, "oma", "Oma zum Geburtstag anrufen")
            .due("2026-11-16")
            .labels(&["familie"])
            .prio(2),
    );
    push(
        v,
        task(306, 3, "brot", "Brot kaufen")
            .state(Progress::Completed, 0)
            .labels(&["einkauf"]),
    );
    push(
        v,
        task(307, 3, "fahrrad", "Fahrradreifen flicken").labels(&["haus"]),
    );
    push(
        v,
        task(308, 3, "buecherei", "Bücher zurückbringen")
            .due("2026-11-14")
            .prio(5),
    );
    push(
        v,
        task(309, 3, "miete", "Miete überweisen")
            .start("2026-12-01")
            .due("2026-12-01")
            .series(every(Frequency::Monthly, 1, RuleEnd::Count(12)))
            .labels(&["geld"]),
    );
    push(
        v,
        task(310, 3, "stammtisch", "Stammtisch (zweiter Dienstag)")
            .due("2026-11-10")
            .original_rule("FREQ=MONTHLY;BYDAY=2TU")
            .labels(&["verein"]),
    );
    push(
        v,
        task(311, 3, "einkauf", "Einkaufsliste Wochenende")
            .due("2026-11-20")
            .labels(&["einkauf"]),
    );
    push(v, task(312, 3, "milch", "Milch").parent(311));
    push(v, task(313, 3, "eier", "Eier").parent(311));
    push(
        v,
        task(314, 3, "kaese", "Käse")
            .parent(311)
            .state(Progress::Completed, 0),
    );
}

pub(crate) fn verein(v: &mut Vec<Pair>) {
    push(
        v,
        task(401, 4, "versammlung", "Mitgliederversammlung vorbereiten")
            .due("2026-12-05")
            .prio(2)
            .labels(&["verein", "planung"])
            .who(&[(5, Accepted), (1, Accepted)])
            .est(1, 0),
    );
    push(
        v,
        task(402, 4, "einladungen", "Einladungen verschicken")
            .due("2026-11-30")
            .dep("tagesordnung", DependsOn)
            .labels(&["verein"]),
    );
    push(
        v,
        task(403, 4, "tagesordnung", "Tagesordnung abstimmen")
            .due("2026-11-25")
            .state(Progress::InProcess, 0)
            .who(&[(6, Open)])
            .labels(&["verein", "planung"]),
    );
    push(
        v,
        task(404, 4, "protokoll", "Protokoll der letzten Sitzung")
            .state(Progress::Completed, 13)
            .labels(&["verein"]),
    );
    let mut kasse = task(405, 4, "kasse", "Kassenprüfung vorbereiten")
        .due("2026-11-23")
        .labels(&["verein", "geld"])
        .pct(0);
    kasse.t.claim = Some(Claim {
        by: person(2),
        until: Utc::from_millis(crate::fake::START_MS + 3 * 86_400_000),
    });
    push(v, kasse);
    push(
        v,
        task(406, 4, "quittungen", "Spendenquittungen schreiben")
            .labels(&["geld"])
            .who(&[(4, Open)]),
    );
}
