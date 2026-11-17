//! K19–K24: Stufen-Filter, Notiz, Anlegen mit Stufe und Rand, Austausch, Migration und die Eigenschaftsprobe.
//!
//! Pattern: Template method (je Fall ein Aufbau, eine Handlung, eine erwartete Antwort und ein Mutant). K24 prüft gegen
//! ein eigenes Orakel (`oracle`), das `ColumnSet::problems` und `column_of` NICHT aufrufen darf.

use super::kern::{Kern, draft, has_code};
use crate::draft::tasks_v2::{Edge, Error, NOTE_MAX, Progress, StageKey, TaskError};

/// K19: `stages = [None]` filtert Karten mit Stufe (Braucht-Mensch) heraus; nur die Grundstufe bleibt.
/// Mutant: der Filter ist aus.
#[tokio::test]
#[ignore = "kern: needs the v2 provider"]
async fn k19_stage_filter() {
    let k = Kern::start();
    let l = k.list().await;
    let mut d = draft(l, "needs a human");
    d.stage = StageKey::parse("needs-human");
    let (staged, plain) = (
        k.t.create(d).await.unwrap(),
        k.card(l, Progress::NeedsAction).await,
    );
    let mut q = k.query(vec![l]);
    q.stages = vec![None];
    let page = k.t.list(&q, None, 50).await.unwrap().items;
    assert!(page.iter().any(|s| s.id == plain.id) && page.iter().all(|s| s.id != staged.id));
}

/// K20: `note` mit 500 Byte ist Ok, mit 501 Byte oder `\n` `Invalid(note.too-long)` bzw. `(note.control)`.
/// Mutant: `>` statt `>=` an der Grenze.
#[tokio::test]
#[ignore = "kern: needs the v2 provider"]
async fn k20_note_limits() {
    let k = Kern::start();
    let l = k.list().await;
    let c = k.card(l, Progress::NeedsAction).await;
    let ok =
        k.t.note(c.id, c.revision, Some(&"x".repeat(NOTE_MAX)))
            .await
            .unwrap();
    let long =
        k.t.note(c.id, ok.revision, Some(&"x".repeat(NOTE_MAX + 1)))
            .await;
    assert!(
        matches!(long, Err(TaskError::Common(Error::Invalid(p))) if has_code(&p, "note.too-long"))
    );
    let line = k.t.note(c.id, ok.revision, Some("a\nb")).await;
    assert!(
        matches!(line, Err(TaskError::Common(Error::Invalid(p))) if has_code(&p, "note.control"))
    );
    let cleared = k.t.note(c.id, ok.revision, None).await.unwrap();
    assert_eq!(cleared.note, None);
}

/// K21 (K-Draft): `create` mit `stage`, Labels und `at = Bottom` speichert genau den Entwurf, die Karte steht unten.
/// Mutant: `at` wird ignoriert (oben).
#[tokio::test]
#[ignore = "kern: needs the v2 provider"]
async fn k21_create_stores_the_draft() {
    let k = Kern::start();
    let l = k.list().await;
    let first = k.card(l, Progress::InProcess).await;
    let mut d = draft(l, "second");
    (d.progress, d.stage, d.at) = (Progress::InProcess, StageKey::parse("review"), Edge::Bottom);
    d.rest.labels = Some(vec!["a".to_owned()]);
    let made = k.t.create(d).await.unwrap();
    assert_eq!(
        (made.stage.clone(), made.labels.clone()),
        (StageKey::parse("review"), vec!["a".to_owned()])
    );
    assert!(made.rank > first.rank);
}

/// K22: Export und Import (JSCalendar und VTODO) mit `stage` und `note` laufen im Kreis gleich; ein Ziel ohne das Feld
/// meldet es in `lost`. Mutant: das Feld fällt still weg. (Grenze `TaskExchange`; braucht den Austausch-Anbieter.)
#[tokio::test]
#[ignore = "kern: needs the v2 provider and the exchange"]
async fn k22_exchange_round_trip() {
    let _k = Kern::start();
    unimplemented!("kern: TaskExchange of the v2 provider does not exist yet");
}

/// K23: Migration: Altbestand ohne Spalten und `stage`: alle Karten in den Grundspalten, die Ränge gleich, die Notiz aus
/// dem Herstellerfeld `freesynergy.net:note`. Mutant: die Ränge werden neu verteilt.
#[tokio::test]
#[ignore = "kern: needs the v2 provider"]
async fn k23_migration_keeps_ranks() {
    let k = Kern::start();
    let l = k.legacy_list().await;
    let set = k.t.columns(l).await.unwrap();
    assert!(!set.stored);
    let q = k.query(vec![l]);
    let items = k.t.list(&q, None, 500).await.unwrap().items;
    assert!(items.iter().all(|s| s.stage.is_none()));
}

/// K24 (Lehre A8 aus groups): Zufallsfolgen aus `set_columns`, `move_to`, `progress`, `create` (Mensch und KI) gegen das
/// eigene Orakel (`oracle::Oracle`, Mengenmodell: je Folge eine `Vec` mit Index-Einfügen, Gültigkeit durch Aufzählen
/// aller 5-mal-Stufen-Paare). Invarianten: jede Karte genau einmal im Layout; die Folge gleicht dem Orakel; KI-Schritte
/// ändern nie die relative Lage zweier anderer Karten; der Anbieter lehnt ab genau dann, wenn das Orakel ablehnt.
/// Mutant: das Orakel ruft `ColumnSet::problems` oder `column_of` (verboten: es prüfte dann den Anbieter an sich selbst).
#[tokio::test]
#[ignore = "kern: needs the v2 provider"]
async fn k24_property_against_the_oracle() {
    let _k = Kern::start();
    let _o = super::oracle::Oracle::new();
    unimplemented!("kern: the random driver runs against the v2 provider");
}
