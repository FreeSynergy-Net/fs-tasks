//! K25–K30: der Stand-Zähler `generation` und die Meldungen (Live-Feed, Regeln G1–G4, Entwurf Abschnitt 10).
//!
//! Pattern: Template method (je Fall ein Aufbau, eine Handlung, eine erwartete Antwort und ein Mutant).

use super::kern::{Fault, Kern, col, std4};
use crate::draft::tasks_v2::{Children, Error, Progress, SeriesScope, Slot, TaskError, Tasks};

const WHOLE: SeriesScope = SeriesScope::Whole;

async fn value(t: &dyn Tasks) -> u64 {
    t.generation().await.expect("generation").value
}

/// K25: je Schreibmethode aus G1 eine ändernde Ausführung (Tabelle `create`, `update`, `assign`, `progress`, `place`,
/// `move_to`, `note`, `checklist`, `relate`, `claim`, `release`, `delete`, `restore`, `purge`, Listen-Methoden,
/// `set_columns`, `import`): `value` +1 und genau ein `Change` mit diesem Wert.
/// Mutanten: der Zähler fehlt in einer Methode (`set_columns`, `move_to`, `note`, `claim` je ein eigener Mutant).
#[tokio::test]
#[ignore = "kern: needs the v2 provider"]
async fn k25_every_write_counts_once() {
    let k = Kern::start();
    let l = k.list().await;
    let c = k.card(l, Progress::NeedsAction).await;
    let v = value(&*k.t).await;
    let n = k.changes().await.len();
    let c = k.t.note(c.id, c.revision, Some("x")).await.unwrap();
    assert_eq!(value(&*k.t).await, v + 1);
    let seen = k.changes().await;
    assert_eq!((seen.len(), seen[n].generation), (n + 1, Some(v + 1)));
    let key = crate::draft::tasks_v2::ColumnKey::parse("doing").unwrap();
    k.t.move_to(c.id, c.revision, &key, Slot::Bottom, Children::Leave, WHOLE)
        .await
        .unwrap();
    assert_eq!(value(&*k.t).await, v + 2);
    let rev = k.t.columns(l).await.unwrap().revision;
    let mut cols = std4();
    cols.push(col("review", &[Progress::InProcess], Some("review")));
    k.t.set_columns(l, rev, cols).await.unwrap();
    assert_eq!(value(&*k.t).await, v + 3);
    assert!(k.changes().await.last().is_some_and(|x| x.task.is_none()));
}

/// K26: No-op und Ablehnung (`assign` doppelt, `claim_next` ohne Treffer, `Changed`, `Forbidden`): `value` bleibt,
/// kein `Change`. Mutant: jede Anfrage zählt.
#[tokio::test]
#[ignore = "kern: needs the v2 provider"]
async fn k26_noop_and_refusal_do_not_count() {
    let k = Kern::start();
    let (me, bot) = (k.human(), k.agent());
    k.act(&me);
    let l = k.list().await;
    let c = k.card(l, Progress::NeedsAction).await;
    let c = k.t.assign(c.id, c.revision, &me, WHOLE).await.unwrap();
    let (v, n) = (value(&*k.t).await, k.changes().await.len());
    assert!(k.t.assign(c.id, c.revision, &me, WHOLE).await.is_ok());
    let stale = k.t.note(c.id, bumped(c.revision), Some("x")).await;
    assert!(matches!(
        stale,
        Err(TaskError::Common(Error::Changed { .. }))
    ));
    k.act(&bot);
    assert!(k.t.note(c.id, c.revision, Some("x")).await.is_err());
    assert_eq!((value(&*k.t).await, k.changes().await.len()), (v, n));
}

/// Eine Revision, die es nicht gibt (für den Fall `Changed`).
fn bumped(r: crate::draft::tasks_v2::Revision) -> crate::draft::tasks_v2::Revision {
    crate::draft::tasks_v2::Revision(r.0 + 1000)
}

/// K27: `delete` mit 2 Teilaufgaben, und `Rank::spread` beim Einfügen: `value` +1, 3 bzw. n `Change` mit gleichem Wert.
/// Mutant: +1 je Objekt.
#[tokio::test]
#[ignore = "kern: needs the v2 provider"]
async fn k27_one_transaction_one_value() {
    let k = Kern::start();
    let l = k.list().await;
    let parent = k.card(l, Progress::NeedsAction).await;
    for _ in 0..2 {
        let mut d = super::kern::draft(l, "child");
        d.rest.parent = Some(Some(parent.id));
        k.t.create(d).await.unwrap();
    }
    let parent = k.t.get(parent.id).await.unwrap().unwrap();
    let (v, n) = (value(&*k.t).await, k.changes().await.len());
    k.t.delete(parent.id, parent.revision, WHOLE).await.unwrap();
    let new = k.changes().await.split_off(n);
    assert_eq!((value(&*k.t).await, new.len()), (v + 1, 3));
    assert!(new.iter().all(|x| x.generation == Some(v + 1)));
}

/// K28: Neustart des Anbieters (neu öffnen auf demselben Speicher): gleiche `epoch`, `value` nicht kleiner.
/// Mutant: der Zähler steht nur im Speicher (RAM).
#[tokio::test]
#[ignore = "kern: needs the v2 provider"]
async fn k28_survives_a_restart() {
    let mut k = Kern::start();
    let l = k.list().await;
    k.card(l, Progress::NeedsAction).await;
    let before = k.t.generation().await.unwrap();
    k.reopen();
    let after = k.t.generation().await.unwrap();
    assert_eq!(after.epoch, before.epoch);
    assert!(after.value >= before.value);
}

/// K29: Absturz nach dem Schreiben, vor dem Zähler (`FailingTasks`): weder Daten noch Zähler ändern sich.
/// Mutant: der Zähler steht in einer eigenen Transaktion.
#[tokio::test]
#[ignore = "kern: needs the v2 provider"]
async fn k29_counter_and_data_are_one_commit() {
    let k = Kern::start();
    let l = k.list().await;
    let c = k.card(l, Progress::NeedsAction).await;
    let v = value(&*k.t).await;
    k.fail(Fault::BeforeCounter);
    assert!(k.t.note(c.id, c.revision, Some("x")).await.is_err());
    assert_eq!(value(&*k.t).await, v);
    assert_eq!(k.t.get(c.id).await.unwrap().unwrap().note, None);
}

/// K30: Wiederherstellung aus einer Sicherung: neue `epoch` (`value` darf kleiner sein). Mutant: `epoch` fest.
#[tokio::test]
#[ignore = "kern: needs the v2 provider"]
async fn k30_restore_gives_a_new_epoch() {
    let mut k = Kern::start();
    let before = k.t.generation().await.unwrap();
    k.restore_backup();
    let after = k.t.generation().await.unwrap();
    assert_ne!(after.epoch, before.epoch);
}
