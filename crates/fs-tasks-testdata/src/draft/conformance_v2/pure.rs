//! Fälle, die ohne Anbieter laufen: Standardspalten (Wert von K1), Regeln B1–B7 am Wertobjekt (K2–K5, K7 als reine
//! Werte), Fähigkeitsliste, Codes, byte-gleiche Dateien, ein `Rank`, Objektsicherheit, das Orakel gegen die Regeln.
//!
//! Pattern: Template method (feste Fälle; hier gegen die Werte des Ordners selbst).

use super::kern::{col, std4};
use super::oracle::{Col, Oracle};
use crate::draft::tasks_v2::{
    CAPABILITIES, COLUMNS_MAX, ColumnProblem, ColumnSet, INVALID_CODES, NOTE_MAX, ObjectId,
    Progress, Revision, StageKey,
};

fn set(cols: Vec<crate::draft::tasks_v2::BoardColumn>) -> ColumnSet {
    ColumnSet {
        list: ObjectId([1; 16]),
        revision: Revision(0),
        stored: true,
        columns: cols,
    }
}

/// K1 als Wert: der Standard hat 4 Spalten, ist gültig, `stored = false`. Mutant: 5 Spalten.
#[test]
fn standard_is_four_valid_columns() {
    let s = ColumnSet::standard(ObjectId([1; 16]), Revision(0));
    assert!(!s.stored && s.problems().is_empty());
    let keys: Vec<_> = s.columns.iter().map(|c| c.key.as_str()).collect();
    assert_eq!(keys, ["todo", "doing", "done", "closed"]);
    assert_eq!(
        s.columns[3].progress,
        [Progress::Failed, Progress::Cancelled]
    );
}

/// K2/K3/K4/K5 als Werte: je Regel B1–B6 die eine `ColumnProblem`; `(InProcess, None)` neben `(InProcess, testing)`
/// ist gültig. Mutant: eine Prüfung aus.
#[test]
fn rules_b1_to_b6_find_each_problem() {
    let mut no_failed = std4();
    no_failed[3] = col("closed", &[Progress::Cancelled], None);
    assert_eq!(
        set(no_failed).problems(),
        [ColumnProblem::ProgressMissing {
            progress: Progress::Failed
        }]
    );
    let mut twice = std4();
    twice.push(col("a", &[Progress::InProcess], Some("testing")));
    twice.push(col("b", &[Progress::InProcess], Some("testing")));
    assert!(
        matches!(set(twice).problems().as_slice(), [ColumnProblem::PairTwice { first, second, .. }]
        if (first.as_str(), second.as_str()) == ("a", "b"))
    );
    let mut ok = std4();
    ok.push(col("a", &[Progress::InProcess], Some("testing")));
    assert_eq!(set(ok).problems(), Vec::new());
    assert_eq!(set(Vec::new()).problems()[0], ColumnProblem::None);
    let many: Vec<_> = (0..=COLUMNS_MAX)
        .map(|i| {
            col(
                &format!("c{i}"),
                &Progress::ALL,
                Some(format!("s{i}").as_str()),
            )
        })
        .collect();
    assert!(
        set(many)
            .problems()
            .contains(&ColumnProblem::TooMany { max: COLUMNS_MAX })
    );
    let mut dup = std4();
    dup.push(col("todo", &[Progress::NeedsAction], Some("x")));
    assert!(matches!(
        set(dup).problems().as_slice(),
        [ColumnProblem::KeyTwice { .. }]
    ));
    let mut blank = std4();
    blank[0].title = " \t".to_owned();
    assert!(matches!(
        set(blank).problems().as_slice(),
        [ColumnProblem::TitleEmpty { .. }]
    ));
    let mut empty = std4();
    empty[0].progress.clear();
    let p = set(empty).problems();
    assert!(p.contains(&ColumnProblem::ProgressEmpty {
        key: crate::draft::tasks_v2::ColumnKey::parse("todo").unwrap()
    }));
    assert!(p.contains(&ColumnProblem::ProgressMissing {
        progress: Progress::NeedsAction
    }));
    let mut twice_in = std4();
    twice_in[1].progress = vec![Progress::InProcess, Progress::InProcess];
    assert!(matches!(
        set(twice_in).problems().as_slice(),
        [ColumnProblem::ProgressTwice { .. }]
    ));
    let mut wip = std4();
    wip[2].wip = Some(0);
    assert!(matches!(
        set(wip).problems().as_slice(),
        [ColumnProblem::WipZero { .. }]
    ));
}

/// K7 (B7) als Wert: genau (p, s), dann (p, None), dann die erste Spalte mit p; nie kein Ergebnis bei gültigem Satz.
/// Mutant: Karte ohne Spalte fehlt (`None`).
#[test]
fn b7_gives_every_card_a_column() {
    let mut cols = std4();
    cols.insert(2, col("review", &[Progress::InProcess], Some("review")));
    cols.insert(
        3,
        col(
            "also",
            &[Progress::InProcess, Progress::NeedsAction],
            Some("other"),
        ),
    );
    cols.insert(0, col("left", &[Progress::InProcess], Some("x")));
    let s = set(cols);
    assert_eq!(s.problems(), Vec::new());
    let of = |p, st: Option<&str>| {
        s.column_of(p, st.and_then(StageKey::parse).as_ref())
            .map(|k| k.as_str().to_owned())
    };
    assert_eq!(
        of(Progress::InProcess, Some("review")).as_deref(),
        Some("review")
    );
    assert_eq!(
        of(Progress::InProcess, Some("gone")).as_deref(),
        Some("doing")
    );
    assert_eq!(of(Progress::InProcess, None).as_deref(), Some("doing"));
    assert_eq!(
        of(Progress::NeedsAction, Some("gone")).as_deref(),
        Some("todo")
    );
    let only_staged = set(vec![col("a", &Progress::ALL, Some("s"))]);
    assert_eq!(
        only_staged
            .column_of(Progress::Failed, None)
            .map(crate::draft::tasks_v2::ColumnKey::as_str),
        Some("a")
    );
    for p in Progress::ALL {
        assert!(s.column_of(p, None).is_some());
    }
    assert!(set(Vec::new()).column_of(Progress::Failed, None).is_none());
}

/// Der Schlüssel `[a-z0-9-]{1,32}`: Grenzen. Mutant: Großbuchstaben oder 33 Zeichen erlaubt.
#[test]
fn keys_have_the_documented_syntax() {
    assert!(StageKey::parse("a").is_some() && StageKey::parse(&"a".repeat(32)).is_some());
    for bad in ["", "A", "a b", "ä", &"a".repeat(33)] {
        assert!(StageKey::parse(bad).is_none(), "{bad:?}");
    }
    assert!(StageKey::parse("needs-human-2").is_some());
}

/// 25 Fähigkeiten, jede einmal; die ersten 22 sind die von v1 (hier die Anzahl), neu `board.columns`, `task.move`,
/// `task.note`. Mutant: eine doppelt oder vergessen.
#[test]
fn capabilities_are_25_and_unique() {
    assert_eq!(CAPABILITIES.len(), 25);
    let mut sorted = CAPABILITIES.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), 25);
    assert_eq!(
        &CAPABILITIES[22..],
        ["board.columns", "task.move", "task.note"]
    );
    assert_eq!(CAPABILITIES[21], "task.history");
}

/// Die sieben `Invalid`-Codes sind verschieden und haben die Form `feld.grund`; die Grenzen stehen wie im Entwurf.
/// Mutant: ein Code doppelt, `NOTE_MAX` anders.
#[test]
fn codes_and_limits() {
    let mut sorted = INVALID_CODES.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), 7);
    assert!(INVALID_CODES.iter().all(|c| c.split_once('.').is_some()));
    assert_eq!((NOTE_MAX, COLUMNS_MAX), (500, 32));
    assert_eq!(
        ColumnProblem::PairTwice {
            progress: Progress::Failed,
            stage: None,
            first: crate::draft::tasks_v2::ColumnKey::parse("a").unwrap(),
            second: crate::draft::tasks_v2::ColumnKey::parse("b").unwrap()
        }
        .code(),
        "column-pair-twice"
    );
}

/// Die Dateien von `tasks_v2/`, die es auch in `tasks_v1/` gibt und die byte-gleich sind: genau SIEBEN (`shared`, `time`,
/// `rule`, `rank`, `list`, `checklist`, `exchange`). Die Liste kommt aus dem Verzeichnis, nicht aus einer festen Auswahl:
/// ein Eingriff in eine der sieben (Mutant) oder eine neue, zufällig gleiche Datei macht die Zahl falsch.
/// Mutant: ein Zeichen in einer der sieben Dateien von v2 ändern.
#[test]
fn exactly_seven_files_are_byte_equal_to_v1() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let (v1, v2) = (src.join("tasks_v1"), src.join("draft/tasks_v2"));
    let mut same = Vec::new();
    for entry in std::fs::read_dir(&v2).expect("tasks_v2 folder") {
        let name = entry.expect("entry").file_name();
        let (old, new) = (std::fs::read(v1.join(&name)), std::fs::read(v2.join(&name)));
        if old.is_ok_and(|old| new.is_ok_and(|new| old == new)) {
            same.push(name.to_string_lossy().into_owned());
        }
    }
    same.sort();
    let want = [
        "checklist.rs",
        "exchange.rs",
        "list.rs",
        "rank.rs",
        "rule.rs",
        "shared.rs",
        "time.rs",
    ];
    assert_eq!(same, want);
}

/// Es gibt genau ein `struct Rank` und ein `const MAX_LEN` (in `rank.rs`), kein Duplikat in v2.
/// Mutant: ein zweites `MAX_LEN` in `board.rs` oder `query.rs`.
#[test]
fn exactly_one_rank_and_max_len() {
    let all = [
        include_str!("../tasks_v2/api.rs"),
        include_str!("../tasks_v2/board.rs"),
        include_str!("../tasks_v2/checklist.rs"),
        include_str!("../tasks_v2/errors.rs"),
        include_str!("../tasks_v2/exchange.rs"),
        include_str!("../tasks_v2/list.rs"),
        include_str!("../tasks_v2/mod.rs"),
        include_str!("../tasks_v2/query.rs"),
        include_str!("../tasks_v2/rank.rs"),
        include_str!("../tasks_v2/rule.rs"),
        include_str!("../tasks_v2/shared.rs"),
        include_str!("../tasks_v2/task.rs"),
        include_str!("../tasks_v2/time.rs"),
    ];
    let count = |needle: &str| all.iter().map(|s| s.matches(needle).count()).sum::<usize>();
    assert_eq!((count("pub struct Rank("), count("const MAX_LEN")), (1, 1));
}

/// `Tasks` ist als `dyn` nutzbar (Probe-Plan: Null-Anbieter als `dyn Tasks`); Kompilierfall.
#[test]
fn tasks_is_object_safe() {
    let _: Option<&dyn crate::draft::tasks_v2::Tasks> = None;
    let _: Option<std::sync::Arc<dyn crate::draft::tasks_v2::Tasks>> = None;
}

/// Das Orakel stimmt in Stichproben mit den Regeln B1–B6 überein (es ist trotzdem eigenständig geschrieben): gültig
/// genau dann, wenn `problems` leer ist. Mutant: das Orakel lässt `PairTwice` durch.
#[test]
fn oracle_agrees_with_the_rules_on_samples() {
    let to = |cols: &[crate::draft::tasks_v2::BoardColumn]| -> Vec<Col> {
        cols.iter()
            .map(|c| Col {
                key: c.key.as_str().to_owned(),
                progress: c.progress.clone(),
                stage: c
                    .stage
                    .as_ref()
                    .map_or(String::new(), |s| s.as_str().to_owned()),
            })
            .collect()
    };
    let mut twice = std4();
    twice.extend([
        col("a", &[Progress::InProcess], Some("t")),
        col("b", &[Progress::InProcess], Some("t")),
    ]);
    let mut missing = std4();
    missing.pop();
    let mut ok = std4();
    ok.push(col("a", &[Progress::InProcess], Some("t")));
    for cols in [std4(), twice, missing, ok, Vec::new()] {
        assert_eq!(Oracle::accepts(&to(&cols)), set(cols).problems().is_empty());
    }
}
