//! Konformitätsfälle K1–K30 von `tasks_v2` (Entwurf Abschnitte 7 und 10): je Regel ein Fall und der Mutant, der ihn
//! rot macht. Skelett der Probe, kein Anbieter.
//!
//! Pattern: Template method (feste Fälle gegen einen `Tasks`-Anbieter; `kern::Kern` sagt, wie man ihn in die
//! Zustände der Fälle bringt).
//!
//! Was LÄUFT: `pure` (Werte des Ordners selbst: Standardspalten, Regeln B1–B7 am Wertobjekt, Fähigkeitsliste, Codes,
//! byte-gleiche Dateien, ein `Rank`, Objektsicherheit). Was `#[ignore = "kern: ..."]` trägt, braucht den v2-Anbieter
//! (`FakeTasks` v2 oder `LocalTasks`, Bahn Kern): der Anbieter fehlt, `Kern::start` ist `unimplemented!`, ein
//! ignorierter Fall wird dadurch nie still grün. Ist der Anbieter da, fällt das `ignore` weg und jeder Fall muss
//! rot werden, wenn man seinen Mutanten einbaut (Mutant-Lauf der Probe-Folge).
//!
//! Aufteilung: `rules_columns` K1–K7, `rules_move` K8–K11 und K13, `rules_agent` K12 und K14–K18, `rules_data` K19–K24,
//! `rules_live` K25–K30.

// The cases are flat lists of steps on a few cards and persons of ONE case; the short names are the persons.
#![allow(clippy::many_single_char_names, clippy::similar_names)]

mod kern;
mod oracle;
mod pure;
mod rules_agent;
mod rules_columns;
mod rules_data;
mod rules_live;
mod rules_move;
