//! The messages of the plan screens: a code and its parameters, the sentence made here in German or English.
//!
//! Pattern: Catalog — a message is data (`Notice`: code + parameters); `notice_text` makes the sentence from the
//! language, as the data folder messages do (`fs_tasks_server::fault_text`); no finished sentence travels.

use fs_tasks_server::Lang;

/// Something the person must be told after typing a task or changing one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notice {
    /// `task.created-outside-view`: made, but the view shown does not hold it; `place` is where it is.
    OutsideView {
        /// What was typed.
        title: String,
        /// The key of the list it is in (`inbox`, `today`, ..., `label:<name>`).
        place: String,
    },
    /// `task.create-failed`: nothing was made.
    CreateFailed {
        /// What was typed (kept, so nothing is lost).
        title: String,
        /// The kind of the refusal.
        kind: String,
        /// What the store said.
        detail: String,
    },
    /// `task.change-failed`: a move or a tick was refused.
    ChangeFailed {
        /// The kind of the refusal.
        kind: String,
        /// What the store said.
        detail: String,
    },
}

impl Notice {
    /// The code of the message.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Notice::OutsideView { .. } => "task.created-outside-view",
            Notice::CreateFailed { .. } => "task.create-failed",
            Notice::ChangeFailed { .. } => "task.change-failed",
        }
    }
}

fn place_name(place: &str, lang: Lang) -> String {
    let (en, de) = match place {
        "inbox" => ("Inbox", "Eingang"),
        "today" => ("Today", "Heute"),
        "upcoming" => ("Upcoming", "Demnächst"),
        "anytime" => ("Anytime", "Jederzeit"),
        "someday" => ("Someday", "Irgendwann"),
        "logbook" => ("Logbook", "Logbuch"),
        "all" => ("All tasks", "Alle Aufgaben"),
        other => return other.strip_prefix("label:").unwrap_or(other).to_string(),
    };
    (if lang == Lang::De { de } else { en }).to_string()
}

/// The sentence for `notice`.
#[must_use]
pub fn notice_text(notice: &Notice, lang: Lang) -> String {
    match (notice, lang) {
        (Notice::OutsideView { title, place }, Lang::En) => format!(
            "Created \"{title}\" in {}, not in the current filter.",
            place_name(place, lang)
        ),
        (Notice::OutsideView { title, place }, Lang::De) => format!(
            "\u{201e}{title}\u{201c} angelegt in {}, nicht im aktuellen Filter.",
            place_name(place, lang)
        ),
        (
            Notice::CreateFailed {
                title,
                kind,
                detail,
            },
            Lang::En,
        ) => {
            format!("Not created ({kind}): {detail}. Your text: \"{title}\"")
        }
        (
            Notice::CreateFailed {
                title,
                kind,
                detail,
            },
            Lang::De,
        ) => {
            format!("Nicht angelegt ({kind}): {detail}. Dein Text: \u{201e}{title}\u{201c}")
        }
        (Notice::ChangeFailed { kind, detail }, Lang::En) => {
            format!("The change was not saved ({kind}): {detail}")
        }
        (Notice::ChangeFailed { kind, detail }, Lang::De) => {
            format!("Die Änderung wurde nicht gespeichert ({kind}): {detail}")
        }
    }
}
