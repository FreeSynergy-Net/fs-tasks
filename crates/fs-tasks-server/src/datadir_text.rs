//! The words for the data folder messages, German and English.
//!
//! Pattern: Catalog — this repository has no message catalog of its own yet (ADR 0057 names MF2 for that); until it
//! has, these few messages live here, keyed by the code of the fault. The library gives code and parameters, the
//! program makes the sentence.

use crate::datadir::{Fault, Tightened};

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
