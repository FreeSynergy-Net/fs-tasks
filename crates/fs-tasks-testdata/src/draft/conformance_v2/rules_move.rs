//! K8–K11 und K13: `move_to` (Spaltenwechsel in einer Transaktion), Stufe beim Stand-Wechsel, Rang der KI.
//!
//! Pattern: Template method (je Fall ein Aufbau, eine Handlung, eine erwartete Antwort und ein Mutant).

use super::kern::{Kern, has_code};
use crate::draft::tasks_v2::{
    Children, Error, HistoryKind, Progress, SeriesScope, Slot, StageKey, TaskError,
};

const WHOLE: SeriesScope = SeriesScope::Whole;

/// K8: `move_to` von Offen nach Testing mit `After(x)`: `progress = InProcess`, `stage = testing`, direkt nach x,
/// EINE neue Revision und EIN `Change`. Mutant: zwei Transaktionen (erst `progress`, dann `place`).
#[tokio::test]
#[ignore = "kern: needs the v2 provider"]
async fn k08_move_is_one_transaction() {
    let k = Kern::start();
    let l = k.list().await;
    let dev = k.dev_columns(l).await;
    let rev = k.t.columns(l).await.unwrap().revision;
    k.t.set_columns(l, rev, dev).await.unwrap();
    let x = k.card(l, Progress::InProcess).await;
    let c = k.card(l, Progress::NeedsAction).await;
    let before = k.changes().await.len();
    let key = crate::draft::tasks_v2::ColumnKey::parse("testing").unwrap();
    let moved =
        k.t.move_to(
            c.id,
            c.revision,
            &key,
            Slot::After(x.id),
            Children::Leave,
            WHOLE,
        )
        .await;
    let moved = moved.unwrap();
    assert_eq!(
        (moved.progress, moved.stage),
        (Progress::InProcess, StageKey::parse("testing"))
    );
    assert!(moved.revision > c.revision);
    assert_eq!(k.order_of(l, &key).await, vec![x.id, c.id]);
    let new = k.changes().await.split_off(before);
    assert_eq!(new.len(), 1);
    assert_eq!(new[0].kind, HistoryKind::Move);
}

/// K9: `move_to` nach Geschlossen bei `Cancelled`: `progress` bleibt `Cancelled` (nur die Stufe und der Platz
/// ändern sich). Mutant: der erste Wert der Spalte wird immer geschrieben (`Failed`).
#[tokio::test]
#[ignore = "kern: needs the v2 provider"]
async fn k09_keeps_a_progress_the_column_shows() {
    let k = Kern::start();
    let l = k.list().await;
    let c = k.card(l, Progress::Cancelled).await;
    let key = crate::draft::tasks_v2::ColumnKey::parse("closed").unwrap();
    let moved =
        k.t.move_to(c.id, c.revision, &key, Slot::Bottom, Children::Leave, WHOLE)
            .await
            .unwrap();
    assert_eq!(moved.progress, Progress::Cancelled);
}

/// K10: `After(y)` mit y in einer anderen Spalte ist `Invalid(after.outside)`, eine unbekannte Spalte
/// `Invalid(column.unknown)`; nichts geschrieben. Mutant: nur die Rangfolge der Liste wird geprüft.
#[tokio::test]
#[ignore = "kern: needs the v2 provider"]
async fn k10_after_and_column_are_checked() {
    let k = Kern::start();
    let l = k.list().await;
    let (a, y) = (
        k.card(l, Progress::NeedsAction).await,
        k.card(l, Progress::Completed).await,
    );
    let todo = crate::draft::tasks_v2::ColumnKey::parse("todo").unwrap();
    let r =
        k.t.move_to(
            a.id,
            a.revision,
            &todo,
            Slot::After(y.id),
            Children::Leave,
            WHOLE,
        )
        .await;
    assert!(
        matches!(r, Err(TaskError::Common(Error::Invalid(p))) if has_code(&p, "after.outside"))
    );
    let r =
        k.t.move_to(
            a.id,
            a.revision,
            &todo,
            Slot::After(a.id),
            Children::Leave,
            WHOLE,
        )
        .await;
    assert!(
        matches!(r, Err(TaskError::Common(Error::Invalid(p))) if has_code(&p, "after.outside"))
    );
    let nowhere = crate::draft::tasks_v2::ColumnKey::parse("nowhere").unwrap();
    let r =
        k.t.move_to(
            a.id,
            a.revision,
            &nowhere,
            Slot::Bottom,
            Children::Leave,
            WHOLE,
        )
        .await;
    assert!(
        matches!(r, Err(TaskError::Common(Error::Invalid(p))) if has_code(&p, "column.unknown"))
    );
    assert_eq!(k.t.get(a.id).await.unwrap().unwrap().revision, a.revision);
}

/// K11: `progress` auf einen anderen Wert bei `stage = testing` setzt `stage = None`; derselbe Wert ändert nichts.
/// Mutant: die Stufe bleibt stehen.
#[tokio::test]
#[ignore = "kern: needs the v2 provider"]
async fn k11_progress_clears_the_stage() {
    let k = Kern::start();
    let l = k.list().await;
    let dev = k.dev_columns(l).await;
    let rev = k.t.columns(l).await.unwrap().revision;
    k.t.set_columns(l, rev, dev).await.unwrap();
    let c = k.card(l, Progress::NeedsAction).await;
    let key = crate::draft::tasks_v2::ColumnKey::parse("testing").unwrap();
    let c =
        k.t.move_to(c.id, c.revision, &key, Slot::Top, Children::Leave, WHOLE)
            .await
            .unwrap();
    let same =
        k.t.progress(
            c.id,
            c.revision,
            Progress::InProcess,
            Children::Leave,
            WHOLE,
        )
        .await
        .unwrap();
    assert_eq!(
        (same.stage.clone(), same.revision),
        (StageKey::parse("testing"), c.revision)
    );
    let done =
        k.t.progress(
            c.id,
            c.revision,
            Progress::Completed,
            Children::Leave,
            WHOLE,
        )
        .await
        .unwrap();
    assert_eq!(done.stage, None);
}

/// K13: eine KI hält die Karte und `move_to(Bottom)` nach Offen: sie steht ganz unten; die Reihenfolge aller anderen
/// Karten ist unverändert. Mutant: die Karte wird oben eingefügt.
#[tokio::test]
#[ignore = "kern: needs the v2 provider"]
async fn k13_agent_cards_go_to_the_bottom() {
    let k = Kern::start();
    let (me, bot) = (k.human(), k.agent());
    k.act(&me);
    let l = k.list().await;
    let (a, b) = (
        k.card(l, Progress::NeedsAction).await,
        k.card(l, Progress::NeedsAction).await,
    );
    let c = k.card(l, Progress::InProcess).await;
    let todo = crate::draft::tasks_v2::ColumnKey::parse("todo").unwrap();
    let before = k.order_of(l, &todo).await;
    k.act(&bot);
    let until = crate::draft::tasks_v2::Utc::from_millis(4_102_444_800_000);
    let held = k.t.claim(c.id, c.revision, until).await.unwrap();
    k.t.move_to(
        c.id,
        held.revision,
        &todo,
        Slot::Bottom,
        Children::Leave,
        WHOLE,
    )
    .await
    .unwrap();
    assert_eq!(
        k.order_of(l, &todo).await,
        [before.clone(), vec![c.id]].concat()
    );
    assert_eq!(before, vec![a.id, b.id]);
}
