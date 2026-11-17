//! K12 und K14–K18: Regel KI-ORDNUNG (die Reihenfolge gehört den Menschen; die KI schreibt nur, was sie hält) und
//! `claim_next` nach Nutzer-Reihenfolge.
//!
//! Pattern: Template method (je Fall ein Aufbau, eine Handlung, eine erwartete Antwort und ein Mutant).

use super::kern::{Kern, draft};
use crate::draft::tasks_v2::{
    Children, ColumnKey, Edge, Error, Order, ParentFilter, Progress, SeriesScope, Slot, TaskError,
    Utc,
};

fn forbidden(r: &Result<impl std::fmt::Debug, TaskError>, capability: &str) -> bool {
    matches!(r, Err(TaskError::Common(Error::Forbidden { capability: c })) if c == capability)
}

fn until() -> Utc {
    Utc::from_millis(4_102_444_800_000)
}

/// K12: eine KI: `place`, `move_to(Top)`, `create(at: Top)` sind `Forbidden { task.place }`, nichts geschrieben.
/// Mutant: die KI-Prüfung ist aus.
#[tokio::test]
#[ignore = "kern: needs the v2 provider"]
async fn k12_agent_cannot_order() {
    let k = Kern::start();
    let (me, bot) = (k.human(), k.agent());
    k.act(&me);
    let l = k.list().await;
    let c = k.card(l, Progress::NeedsAction).await;
    k.act(&bot);
    let todo = ColumnKey::parse("todo").unwrap();
    assert!(forbidden(
        &k.t.place(c.id, c.revision, None).await,
        "task.place"
    ));
    let r =
        k.t.move_to(
            c.id,
            c.revision,
            &todo,
            Slot::Top,
            Children::Leave,
            SeriesScope::Whole,
        )
        .await;
    assert!(forbidden(&r, "task.place"));
    let mut d = draft(l, "from the agent");
    d.at = Edge::Top;
    assert!(forbidden(&k.t.create(d).await, "task.place"));
    assert_eq!(k.t.get(c.id).await.unwrap().unwrap().revision, c.revision);
}

/// K14: eine KI ohne eigenen Claim: `note` und `progress` sind `Forbidden { task.claim }`; hält eine andere, `Claimed`.
/// Mutant: der Claim wird nicht geprüft.
#[tokio::test]
#[ignore = "kern: needs the v2 provider"]
async fn k14_agent_writes_only_what_it_holds() {
    let k = Kern::start();
    let (me, bot) = (k.human(), k.agent());
    k.act(&me);
    let l = k.list().await;
    let c = k.card(l, Progress::NeedsAction).await;
    k.act(&bot);
    assert!(forbidden(
        &k.t.note(c.id, c.revision, Some("x")).await,
        "task.claim"
    ));
    let r =
        k.t.progress(
            c.id,
            c.revision,
            Progress::InProcess,
            Children::Leave,
            SeriesScope::Whole,
        )
        .await;
    assert!(forbidden(&r, "task.claim"));
    k.act(&me);
    let held = k.t.claim(c.id, c.revision, until()).await.unwrap();
    k.act(&bot);
    let r = k.t.note(c.id, held.revision, Some("x")).await;
    assert!(matches!(r, Err(TaskError::Claimed(_))));
}

/// K15: ein Akteur „KI-Status unbekannt“ wird wie in K12 behandelt (fail-closed). Mutant: unbekannt = Mensch.
#[tokio::test]
#[ignore = "kern: needs the v2 provider"]
async fn k15_unknown_actor_counts_as_agent() {
    let k = Kern::start();
    let (me, who) = (k.human(), k.unknown());
    k.act(&me);
    let l = k.list().await;
    let c = k.card(l, Progress::NeedsAction).await;
    k.act(&who);
    assert!(forbidden(
        &k.t.place(c.id, c.revision, None).await,
        "task.place"
    ));
}

/// K16: `claim_next` (Regel KI-ORDNUNG 5): der Mensch hat C über A gezogen, A ist fälliger: die Karte C.
/// Mutant: Sortierung nach `due`.
#[tokio::test]
#[ignore = "kern: needs the v2 provider"]
async fn k16_next_is_the_top_card() {
    let k = Kern::start();
    let (me, bot) = (k.human(), k.agent());
    k.act(&me);
    let l = k.list().await;
    let (a, _b, c) = (
        k.card(l, Progress::NeedsAction).await,
        k.card(l, Progress::NeedsAction).await,
        k.card(l, Progress::NeedsAction).await,
    );
    k.t.place(c.id, c.revision, None).await.unwrap();
    k.act(&bot);
    let mut q = k.query(vec![l]);
    (q.progress, q.stages, q.parent, q.order) = (
        vec![Progress::NeedsAction],
        vec![None],
        Some(ParentFilter::TopLevel),
        Order::Rank,
    );
    let got = k.t.claim_next(&q, until()).await.unwrap().unwrap();
    assert_eq!(got.id, c.id);
    assert_ne!(got.id, a.id);
}

/// K17: `claim_next` über die Listen `[L2, L1]`: die oberste Karte aus L2 vor L1. Mutant: Listen nach Id.
#[tokio::test]
#[ignore = "kern: needs the v2 provider"]
async fn k17_lists_in_the_given_order() {
    let k = Kern::start();
    let (l1, l2) = (k.list().await, k.list().await);
    let (_c1, c2) = (
        k.card(l1, Progress::NeedsAction).await,
        k.card(l2, Progress::NeedsAction).await,
    );
    let mut q = k.query(vec![l2, l1]);
    (q.parent, q.order) = (Some(ParentFilter::TopLevel), Order::Rank);
    let got = k.t.claim_next(&q, until()).await.unwrap().unwrap();
    assert_eq!(got.id, c2.id);
}

/// K18: `claim_next` mit `Order::Rank` und `lists = []` bzw. `parent = None` ist `Invalid`
/// (`lists.empty-for-rank`, `parent.missing-for-rank`). Mutant: Ränge quer über Folgen sortiert.
#[tokio::test]
#[ignore = "kern: needs the v2 provider"]
async fn k18_rank_needs_one_sequence() {
    let k = Kern::start();
    let l = k.list().await;
    let mut q = k.query(Vec::new());
    (q.parent, q.order) = (Some(ParentFilter::TopLevel), Order::Rank);
    let r = k.t.claim_next(&q, until()).await;
    assert!(
        matches!(r, Err(TaskError::Common(Error::Invalid(p))) if super::kern::has_code(&p, "lists.empty-for-rank"))
    );
    let mut q = k.query(vec![l]);
    (q.parent, q.order) = (None, Order::Rank);
    let r = k.t.claim_next(&q, until()).await;
    assert!(
        matches!(r, Err(TaskError::Common(Error::Invalid(p))) if super::kern::has_code(&p, "parent.missing-for-rank"))
    );
}
