//! Why the server could not be reached: a code and parameters, never a bare sentence.
//!
//! Pattern: Value object — the library gives the code and the parameters, the program makes the sentence
//! (`datadir_text`). Every way the start can fail ends here, so no failure is silent: the command line prints
//! `code params` and the sentence, the window shows them.

use std::fmt;
use std::path::PathBuf;

use crate::datadir_text::{Lang, start_text};

/// A server that holds the socket and does not answer, and what is known about its process.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stuck {
    /// The process number; `None` if the socket would not say.
    pub pid: Option<i32>,
    /// The start time of the process in clock ticks since the boot (what a confirmation is checked against).
    pub start: Option<u64>,
    /// When it started, as UTC text.
    pub since: Option<String>,
    /// Its program file.
    pub exe: Option<String>,
    /// Whether the person may say yes to ending it (their own process: the same user, nothing unclear).
    pub can_ask: bool,
    /// The code of the reason it is not ended without asking.
    pub why: &'static str,
}

/// The ways the start fails.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StartError {
    /// The server runs, holds the socket and does not answer (code `server-unresponsive`).
    Unresponsive(Stuck),
    /// Another start holds the start lock and the server does not come up (code `start-locked`).
    StartLocked {
        /// The lock file.
        path: PathBuf,
    },
    /// The socket is served by another program (code `other-program`).
    OtherProgram {
        /// The name the server gave.
        program: String,
    },
    /// Anything else (code `start-failed`); the text says what.
    Failed(String),
}

impl StartError {
    /// The code of the failure.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            StartError::Unresponsive(_) => "server-unresponsive",
            StartError::StartLocked { .. } => "start-locked",
            StartError::OtherProgram { .. } => "other-program",
            StartError::Failed(_) => "start-failed",
        }
    }

    /// The parameters as `name=value` words.
    #[must_use]
    pub fn params(&self) -> String {
        let word = |name: &str, value: Option<String>| {
            value.map(|v| format!("{name}={v}")).unwrap_or_default()
        };
        let words = match self {
            StartError::Unresponsive(s) => vec![
                word("pid", s.pid.map(|p| p.to_string())),
                word("since", s.since.clone()),
                word("exe", s.exe.clone()),
            ],
            StartError::StartLocked { path } => {
                vec![word("path", Some(path.display().to_string()))]
            }
            StartError::OtherProgram { program } => vec![word("program", Some(program.clone()))],
            StartError::Failed(_) => Vec::new(),
        };
        words
            .into_iter()
            .filter(|w| !w.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// The exit code of the window when it ends with this: 3 for a server that does not answer or a start that cannot
    /// proceed, 1 for the rest.
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        match self {
            StartError::Unresponsive(_) | StartError::StartLocked { .. } => 3,
            _ => 1,
        }
    }

    /// The silent server, if that is the failure.
    #[must_use]
    pub fn stuck(&self) -> Option<&Stuck> {
        match self {
            StartError::Unresponsive(stuck) => Some(stuck),
            _ => None,
        }
    }

    /// What goes to stderr: the code and the parameters, then the sentence in `lang`.
    #[must_use]
    pub fn report(&self, lang: Lang) -> String {
        format!(
            "{} {}\n{}",
            self.code(),
            self.params(),
            start_text(self, lang)
        )
        .replace(" \n", "\n")
    }
}

impl fmt::Display for StartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StartError::Failed(text) => f.write_str(text),
            _ => f.write_str(&self.report(Lang::En)),
        }
    }
}

impl std::error::Error for StartError {}
