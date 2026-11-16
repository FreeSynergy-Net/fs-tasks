//! Handreihenfolge als Bruchindex (`Rank`): einsortieren ohne Neu-Nummerieren.
//!
//! Pattern: Value Object (Wert mit Prüfung im Konstruktor; `between` ist eine reine Funktion, kein Speicherzugriff).

/// Ein Platz in der Hand-Reihenfolge: ein Text aus `a`-`z`, der nie auf `a` endet, sortiert lexikographisch (Byte-Reihenfolge,
/// ein Präfix ist kleiner). Wie ein Bruch `0,xyz` zur Basis 26 gelesen: zwischen zwei Plätzen gibt es immer einen dritten.
///
/// Regel (Vertrag, Konformität PLAN §11): `place` berechnet den Rang der Karte mit [`Rank::between`] aus dem Rang der
/// Vorgängerin und dem der Nachfolgerin und schreibt im Normalfall NUR diese eine Aufgabe: kein Neu-Nummerieren, keine
/// Schreibzugriffe auf Nachbarn, keine fremde Revision, keine Zusatz-Einträge in `changes`/`history`. Gleicher Rang
/// (gleichzeitige Abgleiche) steht nach `id` (stabile Seiten). Die Aufgaben einer Rangfolge sind die mit derselben Liste,
/// derselben Eltern-Aufgabe und demselben `progress` (eine Board-Spalte). Am Rand (ganz oben, ganz unten; der Normalfall:
/// neue und erledigte Karten kommen oben hin) geht `between` um einen Schritt weiter (etwa 25 Einfügungen je Zeichen), in
/// der Mitte halbiert es (etwa 4 bis 5 je Zeichen).
///
/// Vergleichbar sind Ränge NUR innerhalb einer Rangfolge (einer Liste, in ihr je Eltern-Aufgabe und Spalte); über Listen
/// hinweg bedeuten sie nichts. Die Länge ist auf [`Rank::MAX_LEN`] (255 Zeichen) begrenzt. Passt zwischen zwei Nachbarn
/// kein Rang mehr, ist das Ergebnis `RankError::Exhausted`; sind die Nachbarn gleich (nach einem Abgleich), ist es
/// `RankError::Equal`. In beiden Fällen verteilt der Anbieter die Rangfolge EINMAL neu ([`Rank::spread`]) und rechnet
/// dann weiter. Das gilt für `create`, `progress` und `place` gleich (alle setzen einen Rang): ein Aufruf, EINE
/// Transaktion, kein `Changed` an den Aufrufer; die anderen Aufgaben der Rangfolge, deren Rang sich ändert, bekommen eine
/// neue Revision und je einen `Change` der Art `Move`. Das ist der einzige Fall, in dem ein Aufruf mehr als eine Aufgabe
/// schreibt; `Invalid` ist nur `RankError::Swapped` (vertauschte Nachbarn). Gleichzeitiges Neu-Verteilen zweier
/// Abgleich-Replikate ist v2.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Rank(String);

/// Warum kein Rang berechnet werden konnte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RankError {
    /// `before > after`: vertauschte Nachbarn (`Invalid`).
    Swapped,
    /// `before == after`: gleiche Nachbarn, die Rangfolge neu verteilen (siehe [`Rank`]).
    Equal,
    /// Das Ergebnis wäre länger als [`Rank::MAX_LEN`]: die Rangfolge neu verteilen (siehe [`Rank`]).
    Exhausted,
}

impl Rank {
    /// Längste erlaubte Rang-Zeichenfolge.
    pub const MAX_LEN: usize = 255;

    /// Prüft den Text: nicht leer, nur `a`-`z`, nicht auf `a` endend.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let ok = !text.is_empty()
            && text.len() <= Self::MAX_LEN
            && text.bytes().all(|b| b.is_ascii_lowercase())
            && !text.ends_with('a');
        ok.then(|| Self(text.to_owned()))
    }

    /// Der Text (zum Speichern in einer typisierten Textspalte).
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Ein Rang echt zwischen `before` (`None` = ganz oben, davor nichts) und `after` (`None` = ganz unten).
    ///
    /// # Errors
    /// `Swapped` bei `before > after`, `Equal` bei `before == after`, `Exhausted`, wenn das Ergebnis zu lang würde.
    pub fn between(before: Option<&Self>, after: Option<&Self>) -> Result<Self, RankError> {
        let low = before.map_or(&b""[..], |r| r.0.as_bytes());
        let high = after.map(|r| r.0.as_bytes());
        if let Some(h) = high {
            match low.cmp(h) {
                std::cmp::Ordering::Greater => return Err(RankError::Swapped),
                std::cmp::Ordering::Equal => return Err(RankError::Equal),
                std::cmp::Ordering::Less => {}
            }
        }
        let text = if before.is_none() && after.is_none() {
            "n".to_owned()
        } else {
            String::from_utf8(midpoint(low, high)).map_err(|_| RankError::Swapped)?
        };
        if text.len() > Self::MAX_LEN {
            return Err(RankError::Exhausted);
        }
        Ok(Self(text))
    }

    /// `count` aufsteigende, gleichmäßig verteilte Ränge (zum einmaligen Neu-Verteilen einer Rangfolge); so kurz wie möglich.
    #[must_use]
    pub fn spread(count: usize) -> Vec<Self> {
        let n = count as u128 + 1;
        let (mut width, mut cap) = (1_u32, 26_u128);
        while cap < n * 26 {
            width += 1;
            cap *= 26;
        }
        let step = cap / n;
        (1..n)
            .map(|i| {
                let mut v = i * step;
                let mut digits = Vec::new();
                for _ in 0..width {
                    digits.push(b'a' + u8::try_from(v % 26).unwrap_or(0));
                    v /= 26;
                }
                digits.reverse();
                while digits.last() == Some(&b'a') {
                    digits.pop();
                }
                Self(String::from_utf8(digits).unwrap_or_default())
            })
            .collect()
    }
}

/// Ziffer 0-25 (`a`-`z`) an Stelle `i`; fehlt sie: `fallback`.
fn digit(s: &[u8], i: usize, fallback: u8) -> u8 {
    s.get(i).map_or(fallback, |b| b - b'a')
}

fn midpoint(low: &[u8], high: Option<&[u8]>) -> Vec<u8> {
    if let Some(h) = high {
        let mut n = 0;
        while n < h.len() && digit(low, n, 0) == digit(h, n, 0) {
            n += 1;
        }
        if n > 0 {
            let rest_low = low.get(n..).unwrap_or(&[]);
            let mut out = h[..n].to_vec();
            out.extend(midpoint(rest_low, Some(&h[n..])));
            return out;
        }
    }
    let da = digit(low, 0, 0);
    match high {
        // Anhängen: ein Schritt weiter (am Ende `z`: `z` und dann weiter).
        None if da < 25 => vec![b'a' + da + 1],
        None => {
            let mut out = vec![b'z'];
            out.extend(midpoint(low.get(1..).unwrap_or(&[]), None));
            out
        }
        // Voranstellen (davor nichts): ein Schritt zurück statt Halbieren.
        Some(h) if low.is_empty() => {
            let d = digit(h, 0, 0);
            if d >= 2 {
                vec![b'a' + d - 1]
            } else if h.len() > 1 {
                vec![h[0]]
            } else {
                b"az".to_vec()
            }
        }
        Some(h) => {
            let db = digit(h, 0, 26);
            if db - da > 1 {
                vec![b'a' + (da + db).div_ceil(2)]
            } else if h.len() > 1 {
                vec![h[0]]
            } else {
                let mut out = vec![b'a' + da];
                out.extend(midpoint(low.get(1..).unwrap_or(&[]), None));
                out
            }
        }
    }
}
