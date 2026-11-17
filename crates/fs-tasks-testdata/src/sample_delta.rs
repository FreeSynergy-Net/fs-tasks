//! The sample of the delta: a list owned by a group, tasks assigned to groups, a claim, checklists, series from
//! completion, and the trash states `Manual` and `Held`.
//!
//! Pattern: Fixed data (one line of builder calls per task; invented people and groups of [`crate::KNOWN`]).

use crate::fake::START_MS;
use crate::sample::{B, task};
use crate::tasks_v1::{
    AssigneeStatus::{Accepted, NeedsAction as Open},
    Checked, ChecklistItem, Claim, Frequency, Rank, Recurrence, RecurrenceRule, RepeatFrom,
    RuleEnd, Task, Utc,
};

type Pair = (&'static str, Task);

fn push(v: &mut Vec<Pair>, b: B) {
    v.push((b.slug, b.t));
}

impl B {
    /// A checklist: the titles in order, the first `ticked` of them ticked by person 1 two days ago.
    pub(crate) fn checklist(mut self, titles: &[&str], ticked: usize) -> Self {
        let ranks = Rank::spread(titles.len());
        for (i, (title, rank)) in titles.iter().zip(ranks).enumerate() {
            self.t.checklist.push(ChecklistItem {
                key: format!("sample-{}-{}", self.t.id.0[15], i + 1),
                title: (*title).to_owned(),
                done: (i < ticked).then(|| Checked {
                    by: crate::ids::person(1),
                    at: Utc::from_millis(START_MS - 2 * 86_400_000),
                }),
                rank,
            });
        }
        self
    }

    /// A series from completion (the same task moves on when it is done): every `interval` days, or months.
    pub(crate) fn repeating_from_completion(mut self, frequency: Frequency, interval: u32) -> Self {
        let rule = RecurrenceRule::every(frequency, interval);
        self.t.recurrence = vec![Recurrence::Rule(RecurrenceRule {
            end: RuleEnd::Never,
            ..rule
        })];
        self.t.repeat_from = RepeatFrom::Completion;
        self
    }

    /// Held by person `by` for `days` more days.
    pub(crate) fn claimed(mut self, by: u64, days: i64) -> Self {
        let who = crate::sample_people::KNOWN
            .iter()
            .find(|k| k.number == by)
            .map_or_else(|| crate::ids::person(by), crate::Known::person);
        self.t.claim = Some(Claim {
            by: who,
            until: Utc::from_millis(START_MS + days * 86_400_000),
        });
        self
    }
}

/// The tasks of the delta (list 3 "Alltag", list 1 "Umzug" and list 6 "Vorstand").
pub(crate) fn delta(v: &mut Vec<Pair>) {
    push(
        v,
        task(701, 6, "jahresversammlung", "Jahresversammlung einladen")
            .due("2026-12-12")
            .prio(2)
            .labels(&["verein"])
            .who(&[(6, Open)]),
    );
    push(
        v,
        task(702, 6, "kassenpruefung", "Kassenprüfung planen")
            .due("2026-12-01")
            .labels(&["verein", "geld"])
            .who(&[(6, Open), (2, Accepted)])
            .claimed(2, 3),
    );
    push(
        v,
        task(703, 6, "vorstandsprotokoll", "Protokoll abheften").labels(&["verein"]),
    );
    push(
        v,
        task(704, 1, "umzugsteam", "Umzugsteam trifft sich")
            .due("2026-11-18")
            .who(&[(7, Open)]),
    );
    push(
        v,
        task(705, 3, "wocheneinkauf", "Wocheneinkauf")
            .due("2026-11-21")
            .labels(&["einkauf"])
            .checklist(&["Brot", "Gemüse", "Nudeln", "Kaffee", "Seife"], 3),
    );
    push(
        v,
        task(706, 3, "blumen", "Blumen gießen")
            .due_at("2026-11-14T09:00:00", "Europe/Berlin")
            .repeating_from_completion(Frequency::Daily, 3)
            .done_on("2026-11-08T09:00:00")
            .done_on("2026-11-11T09:00:00")
            .labels(&["haushalt"]),
    );
    push(
        v,
        task(707, 3, "rauchmelder", "Rauchmelder prüfen")
            .due("2026-11-20")
            .repeating_from_completion(Frequency::Monthly, 6)
            .labels(&["haushalt"]),
    );
    push(
        v,
        task(708, 3, "wasserfilter", "Wasserfilter wechseln")
            .due_at("2026-11-17T18:00:00", "Europe/Berlin")
            .repeating_from_completion(Frequency::Weekly, 4)
            .checklist(&["Alten Filter ausbauen", "Neuen einsetzen"], 0),
    );
}

/// The two trash states beyond a deadline: "never automatically" (list 3) and kept (list 4).
pub(crate) fn trash_states() -> (Pair, Pair) {
    let manual = task(798, 3, "alte-fotos", "Alte Fotos sortieren").due("2026-10-01");
    let held = task(799, 4, "steuer-2019", "Steuerunterlagen 2019 prüfen").labels(&["geld"]);
    ((manual.slug, manual.t), (held.slug, held.t))
}
