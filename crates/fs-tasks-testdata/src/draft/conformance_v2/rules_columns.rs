//! K1–K7: Spalten einer Liste (Standard, Regeln B1–B6, B7, Rechte und Revision von `set_columns`).
//!
//! Pattern: Template method (je Fall ein Aufbau, eine Handlung, eine erwartete Antwort und ein Mutant).

use super::kern::{Kern, col, std4};
use crate::draft::tasks_v2::{ColumnProblem, Error, Progress, StageKey, TaskError};

/// K1: `columns` ohne Einstellung liefert den Standard mit 4 Spalten und `stored = false`.
/// Mutant: 5 Spalten oder `stored = true`.
#[tokio::test]
#[ignore = "kern: needs the v2 provider"]
async fn k01_default_columns() {
    let k = Kern::start();
    let l = k.list().await;
    let set = k.t.columns(l).await.unwrap();
    assert!(!set.stored);
    assert_eq!(set.columns.len(), 4);
    assert!(set.problems().is_empty());
}

/// K2: ein Satz ohne `Failed` ergibt `Columns([ProgressMissing { Failed }])`, nichts geschrieben (`columns` danach
/// weiter `stored = false`). Mutant: die Prüfung B6 ist aus.
#[tokio::test]
#[ignore = "kern: needs the v2 provider"]
async fn k02_progress_missing() {
    let k = Kern::start();
    let l = k.list().await;
    let rev = k.t.columns(l).await.unwrap().revision;
    let mut cols = std4();
    cols[3] = col("closed", &[Progress::Cancelled], None);
    let r = k.t.set_columns(l, rev, cols).await;
    assert!(matches!(r, Err(TaskError::Columns(p))
        if p == [ColumnProblem::ProgressMissing { progress: Progress::Failed }]));
    assert!(!k.t.columns(l).await.unwrap().stored);
}

/// K3: das Paar (`InProcess`, `testing`) in zwei Spalten ergibt `PairTwice { first, second }`.
/// Mutant: es wird nur `key` auf Eindeutigkeit geprüft.
#[tokio::test]
#[ignore = "kern: needs the v2 provider"]
async fn k03_pair_twice() {
    let k = Kern::start();
    let l = k.list().await;
    let rev = k.t.columns(l).await.unwrap().revision;
    let mut cols = std4();
    cols.push(col("t1", &[Progress::InProcess], Some("testing")));
    cols.push(col("t2", &[Progress::InProcess], Some("testing")));
    let r = k.t.set_columns(l, rev, cols).await;
    assert!(matches!(r, Err(TaskError::Columns(p))
        if matches!(p.as_slice(), [ColumnProblem::PairTwice { first, second, .. }]
            if first.as_str() == "t1" && second.as_str() == "t2")));
}

/// K4: (`InProcess`, ohne Stufe) und (`InProcess`, `testing`) nebeneinander sind gültig.
/// Mutant: das Paar zählt nur `progress`.
#[tokio::test]
#[ignore = "kern: needs the v2 provider"]
async fn k04_pairs_differ_by_stage() {
    let k = Kern::start();
    let l = k.list().await;
    let rev = k.t.columns(l).await.unwrap().revision;
    let mut cols = std4();
    cols.insert(2, col("testing", &[Progress::InProcess], Some("testing")));
    let set = k.t.set_columns(l, rev, cols).await.unwrap();
    assert!(set.stored && set.columns.len() == 5);
}

/// K5: je ein Fall für B1 (keine Spalte, 33 Spalten), B2 (Schlüssel doppelt), B3 (Titel leer), B4 (kein Stand,
/// Stand doppelt), B5 (`wip = Some(0)`): je genau eine `ColumnProblem`, nichts geschrieben.
/// Mutant: je Prüfung einzeln aus.
#[tokio::test]
#[ignore = "kern: needs the v2 provider"]
async fn k05_each_rule_has_its_problem() {
    let k = Kern::start();
    let l = k.list().await;
    let rev = k.t.columns(l).await.unwrap().revision;
    let none = k.t.set_columns(l, rev, Vec::new()).await;
    assert!(matches!(none, Err(TaskError::Columns(p)) if p.contains(&ColumnProblem::None)));
    let mut wip0 = std4();
    wip0[0].wip = Some(0);
    let r = k.t.set_columns(l, rev, wip0).await;
    assert!(
        matches!(r, Err(TaskError::Columns(p)) if matches!(p.as_slice(), [ColumnProblem::WipZero { .. }]))
    );
    let mut blank = std4();
    blank[1].title = "  ".to_owned();
    let r = k.t.set_columns(l, rev, blank).await;
    assert!(
        matches!(r, Err(TaskError::Columns(p)) if matches!(p.as_slice(), [ColumnProblem::TitleEmpty { .. }]))
    );
    let mut twice = std4();
    twice.push(col("todo", &[Progress::NeedsAction], Some("x")));
    let r = k.t.set_columns(l, rev, twice).await;
    assert!(
        matches!(r, Err(TaskError::Columns(p)) if p.iter().any(|x| matches!(x, ColumnProblem::KeyTwice { .. })))
    );
    let many = (0..33)
        .map(|i| {
            col(
                &format!("c{i}"),
                &[Progress::NeedsAction],
                Some(&format!("s{i}")),
            )
        })
        .collect();
    let r = k.t.set_columns(l, rev, many).await;
    assert!(
        matches!(r, Err(TaskError::Columns(p)) if p.iter().any(|x| matches!(x, ColumnProblem::TooMany { max: 32 })))
    );
}

/// K6: `set_columns` mit alter Revision ist `Changed`, als nur `Write` und als KI `Forbidden { board.columns }`.
/// Mutant: `expected` wird ignoriert, oder `Write` reicht.
#[tokio::test]
#[ignore = "kern: needs the v2 provider"]
async fn k06_revision_and_rights() {
    let k = Kern::start();
    let (me, writer, bot) = (k.human(), k.human(), k.agent());
    k.act(&me);
    let l = k.list().await;
    let rev = k.t.columns(l).await.unwrap().revision;
    k.t.set_columns(l, rev, std4()).await.unwrap();
    let stale = k.t.set_columns(l, rev, std4()).await;
    assert!(matches!(
        stale,
        Err(TaskError::Common(Error::Changed { .. }))
    ));
    let rev = k.t.columns(l).await.unwrap().revision;
    for who in [&writer, &bot] {
        k.act(who);
        let r = k.t.set_columns(l, rev, std4()).await;
        assert!(
            matches!(r, Err(TaskError::Common(Error::Forbidden { capability })) if capability == "board.columns")
        );
    }
}

/// K7 (B7): eine Karte (`InProcess`, `review`), die Spalte `review` wird gelöscht: die Karte steht in
/// (`InProcess`, ohne Stufe); gibt es auch die nicht, in der ersten Spalte (links) mit `InProcess`.
/// Mutant: die Karte fehlt im Board.
#[tokio::test]
#[ignore = "kern: needs the v2 provider"]
async fn k07_card_is_never_invisible() {
    let k = Kern::start();
    let l = k.list().await;
    let dev = k.dev_columns(l).await;
    let rev = k.t.columns(l).await.unwrap().revision;
    let set = k.t.set_columns(l, rev, dev.clone()).await.unwrap();
    let review = dev
        .iter()
        .find(|c| c.stage == StageKey::parse("review"))
        .unwrap()
        .key
        .clone();
    let card = k.card(l, Progress::NeedsAction).await;
    k.t.move_to(
        card.id,
        card.revision,
        &review,
        crate::draft::tasks_v2::Slot::Bottom,
        crate::draft::tasks_v2::Children::Leave,
        crate::draft::tasks_v2::SeriesScope::Whole,
    )
    .await
    .unwrap();
    let without: Vec<_> = dev.into_iter().filter(|c| c.key != review).collect();
    let set = k.t.set_columns(l, set.revision, without).await.unwrap();
    let doing = set
        .column_of(Progress::InProcess, StageKey::parse("review").as_ref())
        .unwrap();
    assert_eq!(doing.as_str(), "doing");
    assert!(k.order_of(l, doing).await.contains(&card.id));
}
