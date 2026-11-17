//! The words for the data folder messages, German and English.
//!
//! Pattern: Catalog — this repository has no message catalog of its own yet (ADR 0057 names MF2 for that); until it
//! has, these few messages live here, keyed by the code of the fault. The library gives code and parameters, the
//! program makes the sentence.

use crate::datadir::{Fault, Tightened};
use crate::startfail::{StartError, Stuck};

/// The language of a message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    /// German.
    De,
    /// English (the default).
    En,
}

impl Lang {
    /// The language from `LC_ALL`, `LC_MESSAGES`, `LANG` (the first that is set): German if it starts with `de`.
    #[must_use]
    pub fn from_env() -> Lang {
        let value = ["LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .filter_map(|name| std::env::var(name).ok())
            .find(|v| !v.is_empty())
            .unwrap_or_default();
        Lang::from_locale(&value)
    }

    /// The language of a locale name such as `de_DE.UTF-8`.
    #[must_use]
    pub fn from_locale(locale: &str) -> Lang {
        if locale.to_ascii_lowercase().starts_with("de") {
            Lang::De
        } else {
            Lang::En
        }
    }
}

/// The sentence for a refused data folder.
#[must_use]
pub fn fault_text(fault: &Fault, lang: Lang) -> String {
    let path = fault.path().display();
    match (fault, lang) {
        (Fault::Symlink { .. }, Lang::En) => {
            format!(
                "The data folder {path} is a symbolic link; it is not used. Remove the link or choose another folder."
            )
        }
        (Fault::Symlink { .. }, Lang::De) => {
            format!(
                "Der Datenordner {path} ist ein symbolischer Link und wird nicht benutzt. Link entfernen oder anderen Ordner wählen."
            )
        }
        (Fault::NotDirectory { .. }, Lang::En) => {
            format!("The data folder {path} is not a folder; move that file away.")
        }
        (Fault::NotDirectory { .. }, Lang::De) => {
            format!("Der Datenordner {path} ist kein Ordner; die Datei bitte wegräumen.")
        }
        (Fault::ForeignOwner { owner, me, .. }, Lang::En) => format!(
            "The data folder {path} belongs to user {owner}, not to you ({me}); it is not changed and not used."
        ),
        (Fault::ForeignOwner { owner, me, .. }, Lang::De) => format!(
            "Der Datenordner {path} gehört Benutzer {owner}, nicht dir ({me}); er wird weder geändert noch benutzt."
        ),
        (Fault::Ancestor { ancestor, .. }, Lang::En) => format!(
            "The data folder {path} is not used: the folder {} above it can be changed by others (it must belong to you or to root, and not be writable by others).",
            ancestor.display()
        ),
        (Fault::Ancestor { ancestor, .. }, Lang::De) => format!(
            "Der Datenordner {path} wird nicht benutzt: der Ordner {} darüber kann von anderen verändert werden (er muss dir oder root gehören und darf für andere nicht beschreibbar sein).",
            ancestor.display()
        ),
        (Fault::Io { kind, .. }, Lang::En) => {
            format!("The data folder {path} could not be checked or made private ({kind:?}).")
        }
        (Fault::Io { kind, .. }, Lang::De) => {
            format!("Der Datenordner {path} ließ sich nicht prüfen oder schließen ({kind:?}).")
        }
        (Fault::StillOpen { mode, .. }, Lang::En) => format!(
            "The data folder {path} is still open to others (mode {:04o}) after the change; the file system may not support modes.",
            mode & 0o7777
        ),
        (Fault::StillOpen { mode, .. }, Lang::De) => format!(
            "Der Datenordner {path} ist nach der Änderung noch für andere offen (Modus {:04o}); das Dateisystem kennt womöglich keine Modi.",
            mode & 0o7777
        ),
    }
}

/// The sentence for a folder that was open and is private now.
#[must_use]
pub fn tightened_text(done: &Tightened, lang: Lang) -> String {
    let path = done.path.display();
    let mode = done.old_mode & 0o7777;
    match lang {
        Lang::En => format!(
            "The data folder {path} was open to others (mode {mode:04o}); it is private (0700) now. It may have been read meanwhile."
        ),
        Lang::De => format!(
            "Der Datenordner {path} war für andere offen (Modus {mode:04o}); er ist jetzt privat (0700). Er kann zwischenzeitlich gelesen worden sein."
        ),
    }
}

/// The sentence for a failed start. The words differ by language; the code and parameters (`StartError::report`) do not.
#[must_use]
pub fn start_text(error: &StartError, lang: Lang) -> String {
    match (error, lang) {
        (StartError::Failed(text), _) => text.clone(),
        (StartError::Unresponsive(stuck), lang) => unresponsive_text(stuck, lang),
        (StartError::StartLocked { path }, Lang::En) => format!(
            "Another start holds {} and the server does not come up. Wait a moment and start again.",
            path.display()
        ),
        (StartError::StartLocked { path }, Lang::De) => format!(
            "Ein anderer Start hält {} und der Server kommt nicht hoch. Einen Moment warten und neu starten.",
            path.display()
        ),
        (StartError::OtherProgram { program }, Lang::En) => format!(
            "The socket is served by {program}, not by fs-tasks: check the --socket / --connect argument."
        ),
        (StartError::OtherProgram { program }, Lang::De) => format!(
            "Der Socket gehört {program}, nicht fs-tasks: das Argument --socket / --connect prüfen."
        ),
    }
}

fn process_words(stuck: &Stuck) -> String {
    let word = |name: &str, value: Option<String>| {
        value.map(|v| format!(" {name} {v}")).unwrap_or_default()
    };
    [
        word("PID", stuck.pid.map(|p| p.to_string())),
        word("since", stuck.since.clone()),
        word("program", stuck.exe.clone()),
    ]
    .concat()
    .trim_start()
    .to_owned()
}

fn unresponsive_text(stuck: &Stuck, lang: Lang) -> String {
    let who = process_words(stuck);
    match (stuck.can_ask, lang) {
        (true, Lang::En) => {
            format!("Tasks is already running, but it does not answer ({who}). End it?")
        }
        (true, Lang::De) => {
            format!("Tasks l\u{e4}uft schon, reagiert aber nicht ({who}). Beenden?")
        }
        (false, Lang::En) => format!(
            "Tasks is already running, but it does not answer ({who}). It is not ended from here (reason: {}); end it yourself.",
            stuck.why
        ),
        (false, Lang::De) => format!(
            "Tasks l\u{e4}uft schon, reagiert aber nicht ({who}). Es wird von hier nicht beendet (Grund: {}); bitte selbst beenden.",
            stuck.why
        ),
    }
}

/// The word of the only entry of the window when nothing may be ended.
#[must_use]
pub fn close_word(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Close",
        Lang::De => "Schlie\u{df}en",
    }
}

/// The words of the window that shows a silent server: the title, the answer "yes" (end it, start again) and the answer
/// "no" (close). Menu entries stand for the two answers: there is no question dialog in the shell yet.
#[must_use]
pub fn window_words(lang: Lang) -> (&'static str, &'static str, &'static str) {
    match lang {
        Lang::En => (
            "Tasks does not answer",
            "Yes: end it and start again",
            "No: close",
        ),
        Lang::De => (
            "Tasks antwortet nicht",
            "Ja: beenden und neu starten",
            "Nein: schlie\u{df}en",
        ),
    }
}
