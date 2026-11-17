//! Das Orakel von K24: ein eigenes Mengenmodell des Boards, unabhängig von `board.rs` geschrieben.
//!
//! Pattern: Test oracle (Referenzmodell; je Folge `(progress)` eine `Vec` mit Index-Einfügen, Gültigkeit durch Aufzählen
//! aller Paare). Es ruft `ColumnSet::problems` und `column_of` NICHT auf (sonst prüfte der Anbieter sich selbst).
//!
//! ANNAHMEN, die der Entwurf offen lässt (siehe Probe-Bericht, Frage 1): die Ansicht einer Spalte ist die Folge der
//! Karten ihres Platzes, zuerst nach der Reihenfolge von `progress` der Spalte, dann nach dem Rang; `After(x)` mit einem
//! anderen `progress` als dem geschriebenen heißt „oben“, wenn der Stand von x in der Spalte früher steht, sonst „unten“.

use crate::draft::tasks_v2::Progress;
use std::collections::HashMap;

/// Eine Spalte des Orakels: Schlüssel, Stände in Reihenfolge, Stufe (leer = Grund).
#[derive(Clone, Debug)]
pub(super) struct Col {
    pub(super) key: String,
    pub(super) progress: Vec<Progress>,
    pub(super) stage: String,
}

/// Eine Karte: Id, Stand, Stufe.
#[derive(Clone, Debug)]
pub(super) struct Card {
    pub(super) id: u32,
    pub(super) progress: Progress,
    pub(super) stage: String,
}

/// Der Stand der Welt: Spalten, Karten, je Stand die Folge der Ids.
pub(super) struct Oracle {
    pub(super) cols: Vec<Col>,
    pub(super) cards: Vec<Card>,
    seq: HashMap<usize, Vec<u32>>,
}

fn idx(p: Progress) -> usize {
    Progress::ALL
        .iter()
        .position(|x| *x == p)
        .expect("one of five")
}

impl Oracle {
    pub(super) fn new() -> Self {
        let std = |k: &str, p: &[Progress]| Col {
            key: k.to_owned(),
            progress: p.to_vec(),
            stage: String::new(),
        };
        Self {
            cols: vec![
                std("todo", &[Progress::NeedsAction]),
                std("doing", &[Progress::InProcess]),
                std("done", &[Progress::Completed]),
                std("closed", &[Progress::Failed, Progress::Cancelled]),
            ],
            cards: Vec::new(),
            seq: HashMap::new(),
        }
    }

    /// Gültigkeit durch Aufzählen: 1 bis 32 Spalten, Schlüssel eindeutig, jede Spalte hat Stände ohne Doppel, jeder der
    /// fünf Stände steht irgendwo, jedes Paar (Stand, Stufe) höchstens einmal.
    pub(super) fn accepts(cols: &[Col]) -> bool {
        if cols.is_empty() || cols.len() > 32 {
            return false;
        }
        let key_twice = cols
            .iter()
            .enumerate()
            .any(|(i, c)| cols[..i].iter().any(|d| d.key == c.key));
        let own_twice = cols.iter().any(|c| {
            c.progress.is_empty()
                || (0..c.progress.len()).any(|i| c.progress[..i].contains(&c.progress[i]))
        });
        let missing = Progress::ALL
            .iter()
            .any(|p| !cols.iter().any(|c| c.progress.contains(p)));
        let pair_twice = Progress::ALL.iter().any(|p| {
            let stages: Vec<&str> = cols
                .iter()
                .filter(|c| c.progress.contains(p))
                .map(|c| c.stage.as_str())
                .collect();
            (0..stages.len()).any(|i| stages[..i].contains(&stages[i]))
        });
        !(key_twice || own_twice || missing || pair_twice)
    }

    /// Setzt die Spalten, wenn der Satz gültig ist; sonst `false` und nichts ändert sich.
    pub(super) fn set_columns(&mut self, cols: Vec<Col>) -> bool {
        let ok = Self::accepts(&cols);
        if ok {
            self.cols = cols;
        }
        ok
    }

    /// Der Platz einer Karte (B7), durch Aufzählen.
    pub(super) fn place_of(&self, c: &Card) -> usize {
        let mut best: Option<(u8, usize)> = None;
        for (i, col) in self.cols.iter().enumerate() {
            if !col.progress.contains(&c.progress) {
                continue;
            }
            let rank = if col.stage == c.stage {
                0
            } else if col.stage.is_empty() {
                1
            } else {
                2
            };
            if best.is_none_or(|(r, _)| rank < r) {
                best = Some((rank, i));
            }
        }
        best.expect("a valid set has a place for every progress").1
    }

    /// Die Karten einer Spalte von oben nach unten.
    pub(super) fn layout(&self, col: usize) -> Vec<u32> {
        let mut out = Vec::new();
        for p in &self.cols[col].progress {
            let ids = self
                .seq
                .get(&idx(*p))
                .map(Vec::as_slice)
                .unwrap_or_default();
            out.extend(
                ids.iter()
                    .filter(|id| self.place_of(self.card(**id)) == col),
            );
        }
        out
    }

    pub(super) fn card(&self, id: u32) -> &Card {
        self.cards
            .iter()
            .find(|c| c.id == id)
            .expect("a known card")
    }

    /// Eine neue Karte am Rand ihrer Folge.
    pub(super) fn create(&mut self, id: u32, progress: Progress, stage: &str, top: bool) {
        self.cards.push(Card {
            id,
            progress,
            stage: stage.to_owned(),
        });
        let s = self.seq.entry(idx(progress)).or_default();
        if top { s.insert(0, id) } else { s.push(id) }
    }

    /// Stand ändern: ein anderer Wert leert die Stufe, die Karte kommt an den Rand ihrer neuen Folge (`top`).
    pub(super) fn progress(&mut self, id: u32, to: Progress, top: bool) {
        let c = self.card(id).clone();
        if c.progress == to {
            return;
        }
        self.seq
            .entry(idx(c.progress))
            .or_default()
            .retain(|x| *x != id);
        let s = self.seq.entry(idx(to)).or_default();
        if top {
            s.insert(0, id)
        } else {
            s.push(id)
        }
        let card = self.cards.iter_mut().find(|x| x.id == id).expect("known");
        (card.progress, card.stage) = (to, String::new());
    }
}

/// Wohin `move_to` einfügt (wie `Slot`, mit der Karten-Nummer des Orakels).
#[derive(Clone, Copy, Debug)]
pub(super) enum At {
    Top,
    Bottom,
    After(u32),
}

impl Oracle {
    /// `move_to` des Orakels; `false` = abgelehnt (`After(x)` steht nicht in der Zielspalte), nichts ändert sich.
    pub(super) fn move_to(&mut self, id: u32, col: usize, at: At) -> bool {
        let target = &self.cols[col];
        let c = self.card(id).clone();
        let written = if target.progress.contains(&c.progress) {
            c.progress
        } else {
            target.progress[0]
        };
        if let At::After(x) = at {
            if x == id || !self.layout(col).contains(&x) {
                return false;
            }
        }
        let stage = target.stage.clone();
        let order = target.progress.clone();
        self.seq
            .entry(idx(c.progress))
            .or_default()
            .retain(|v| *v != id);
        let card = self.cards.iter_mut().find(|v| v.id == id).expect("known");
        (card.progress, card.stage) = (written, stage);
        let members: Vec<u32> = self.layout(col).into_iter().filter(|v| *v != id).collect();
        let seq = self.seq.entry(idx(written)).or_default();
        let pos = |seq: &Vec<u32>, m: Option<&u32>| m.and_then(|m| seq.iter().position(|v| v == m));
        let at_index = match at {
            At::Top => pos(seq, members.iter().find(|m| seq.contains(m))).unwrap_or(0),
            At::Bottom => pos(seq, members.iter().rev().find(|m| seq.contains(m)))
                .map_or(seq.len(), |i| i + 1),
            At::After(x) => match pos(seq, Some(&x)) {
                Some(i) => i + 1,
                None => {
                    let xp = self
                        .cards
                        .iter()
                        .find(|v| v.id == x)
                        .map(|v| v.progress)
                        .expect("known");
                    let earlier = order.iter().position(|p| *p == xp)
                        < order.iter().position(|p| *p == written);
                    if earlier { 0 } else { seq.len() }
                }
            },
        };
        self.seq
            .entry(idx(written))
            .or_default()
            .insert(at_index, id);
        true
    }
}
