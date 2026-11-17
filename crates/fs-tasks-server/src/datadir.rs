//! The data folder of the program: made private, and corrected if it was made open.
//!
//! Pattern: Policy + Guard — `decide` is a pure rule over the facts of a folder (testable with made-up facts);
//! `tighten` applies it to a real folder. Up to 0.0.8 the window made the folder with the default mode (0755 by
//! the usual umask) before the server ran; the transport of `fs-interfaces` then refused the folder (it wants
//! 0700), and the program did not start. Now the window makes it 0700, and the server takes bits away from a
//! folder of its own that is too open. It never gives bits, never touches a folder of somebody else, never follows
//! a link and never changes what is inside. The errors are typed (a code and parameters); the words live in
//! `datadir_text`.

use std::io;
use std::path::{Path, PathBuf};

/// The facts about a folder that the rule needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Meta {
    /// It is a directory (not a file, not a link).
    pub is_dir: bool,
    /// The path itself is a symbolic link.
    pub is_symlink: bool,
    /// The user id of the owner.
    pub uid: u32,
    /// The mode bits (only the lowest nine are looked at).
    pub mode: u32,
}

/// Why a data folder is refused. The code and the parameters are the contract; the sentence is made by the program.
#[derive(Debug, PartialEq, Eq)]
pub enum Fault {
    /// The path is a symbolic link.
    Symlink {
        /// The path.
        path: PathBuf,
    },
    /// The path is not a directory.
    NotDirectory {
        /// The path.
        path: PathBuf,
    },
    /// The folder belongs to somebody else.
    ForeignOwner {
        /// The path.
        path: PathBuf,
        /// The owner of the folder.
        owner: u32,
        /// The user this program runs as.
        me: u32,
    },
    /// A folder above the data folder can be changed by others (the transport refuses such a path).
    Ancestor {
        /// The data folder.
        path: PathBuf,
        /// The folder above it that is open.
        ancestor: PathBuf,
    },
    /// The mode could not be read, or the change failed.
    Io {
        /// The path.
        path: PathBuf,
        /// What failed.
        kind: io::ErrorKind,
    },
    /// The change was made, but the folder is still open (a file system without the mode, or an ACL).
    StillOpen {
        /// The path.
        path: PathBuf,
        /// The mode after the change.
        mode: u32,
    },
}

impl Fault {
    /// The stable code of the fault.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Fault::Symlink { .. } => "data-dir-symlink",
            Fault::NotDirectory { .. } => "data-dir-not-directory",
            Fault::ForeignOwner { .. } => "data-dir-foreign-owner",
            Fault::Ancestor { .. } => "data-dir-ancestor",
            Fault::Io { .. } => "data-dir-io",
            Fault::StillOpen { .. } => "data-dir-still-open",
        }
    }

    /// The path the fault is about.
    #[must_use]
    pub fn path(&self) -> &Path {
        match self {
            Fault::Symlink { path }
            | Fault::NotDirectory { path }
            | Fault::ForeignOwner { path, .. }
            | Fault::Ancestor { path, .. }
            | Fault::Io { path, .. }
            | Fault::StillOpen { path, .. } => path,
        }
    }

    /// The code with its parameters in one line, for the log: `code path=... owner=...`.
    #[must_use]
    pub fn params(&self) -> String {
        let base = format!("{} path={}", self.code(), self.path().display());
        match self {
            Fault::ForeignOwner { owner, me, .. } => format!("{base} owner={owner} me={me}"),
            Fault::Ancestor { ancestor, .. } => {
                format!("{base} ancestor={}", ancestor.display())
            }
            Fault::Io { kind, .. } => format!("{base} kind={kind:?}"),
            Fault::StillOpen { mode, .. } => format!("{base} mode={:04o}", mode & 0o7777),
            Fault::Symlink { .. } | Fault::NotDirectory { .. } => base,
        }
    }
}

/// What to do with a folder.
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    /// Leave it: it is private already.
    Keep,
    /// Take bits away: the new mode.
    Tighten(u32),
    /// Refuse it.
    Refuse(Fault),
}

/// The rule. Only a folder of this user that is not a link is ever changed, and only by taking bits away.
#[must_use]
pub fn decide(path: &Path, meta: &Meta, me: u32) -> Action {
    let path = path.to_path_buf();
    if meta.is_symlink {
        Action::Refuse(Fault::Symlink { path })
    } else if !meta.is_dir {
        Action::Refuse(Fault::NotDirectory { path })
    } else if meta.uid != me {
        Action::Refuse(Fault::ForeignOwner {
            path,
            owner: meta.uid,
            me,
        })
    } else if meta.mode & 0o077 != 0 {
        Action::Tighten(meta.mode & 0o700)
    } else {
        Action::Keep
    }
}

/// A folder that was too open and is private now.
#[derive(Debug, PartialEq, Eq)]
pub struct Tightened {
    /// The folder.
    pub path: PathBuf,
    /// Its mode before.
    pub old_mode: u32,
}

impl Tightened {
    /// The line for the log: code, folder, old mode.
    #[must_use]
    pub fn params(&self) -> String {
        format!(
            "data-dir-tightened path={} old_mode={:04o}",
            self.path.display(),
            self.old_mode & 0o7777
        )
    }
}

/// Make the data folder `path` private if it exists and is too open. `Ok(None)`: nothing to do (private already, or
/// not there yet: the transport makes it with 0700). Only this folder is changed, not what is inside and not the
/// folders above (the transport still checks those).
///
/// # Errors
/// The fault says why the folder is refused or could not be corrected.
#[cfg(unix)]
pub fn tighten(path: &Path) -> Result<Option<Tightened>, Fault> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let io = |e: io::Error| Fault::Io {
        path: path.to_path_buf(),
        kind: e.kind(),
    };
    let meta_of = |m: &std::fs::Metadata| Meta {
        is_dir: m.is_dir(),
        is_symlink: m.file_type().is_symlink(),
        uid: m.uid(),
        mode: m.mode(),
    };
    let me = rustix::process::geteuid().as_raw();
    let first = match std::fs::symlink_metadata(path) {
        Ok(m) => meta_of(&m),
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(io(e)),
    };
    let new_mode = match decide(path, &first, me) {
        Action::Refuse(fault) => return Err(fault),
        Action::Keep => return Ok(None),
        Action::Tighten(mode) => mode,
    };
    // Open the folder itself (no link, must be a directory) and work on the descriptor: no path race.
    let file = open_dir_nofollow(path).map_err(io)?;
    // What was opened is looked at again: still a folder of ours, still open.
    let opened = meta_of(&file.metadata().map_err(io)?);
    match decide(path, &opened, me) {
        Action::Refuse(fault) => return Err(fault),
        Action::Keep => return Ok(None),
        Action::Tighten(_) => {}
    }
    file.set_permissions(std::fs::Permissions::from_mode(
        new_mode & opened.mode & 0o700,
    ))
    .map_err(io)?;
    let after = file.metadata().map_err(io)?.mode();
    if after & 0o077 != 0 {
        return Err(Fault::StillOpen {
            path: path.to_path_buf(),
            mode: after,
        });
    }
    Ok(Some(Tightened {
        path: path.to_path_buf(),
        old_mode: opened.mode,
    }))
}

/// Open the folder at `path` itself: a symbolic link (at the last place) or something that is not a folder is an error.
///
/// # Errors
/// The system error (a link: `ELOOP`, a file: `ENOTDIR`).
#[cfg(unix)]
pub(crate) fn open_dir_nofollow(path: &Path) -> io::Result<std::fs::File> {
    use rustix::fs::{Mode, OFlags};
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let fd = rustix::fs::open(path, flags, Mode::empty())?;
    Ok(std::fs::File::from(fd))
}

/// No modes outside Unix (the folder is protected by the ACL of the user profile): nothing to correct.
///
/// # Errors
/// Never.
#[cfg(not(unix))]
pub fn tighten(_path: &Path) -> Result<Option<Tightened>, Fault> {
    Ok(None)
}

/// Make the folder `path` (and the missing folders above it) so that nobody else can read it: 0700 for the folder and
/// for every missing folder from `freesynergy` down; a missing folder above that (`~/.local/share`) gets the
/// ordinary mode. A folder that exists is left as it is (`tighten` corrects the data folder).
///
/// # Errors
/// The text of the system error, with the folder.
pub fn create_private_dir(path: &Path) -> Result<(), String> {
    let named = |dir: &Path, e: io::Error| format!("{}: {e}", dir.display());
    let mut missing = Vec::new();
    let mut at = Some(path);
    while let Some(dir) = at.filter(|d| !d.as_os_str().is_empty() && !d.exists()) {
        missing.push(dir);
        at = dir.parent();
    }
    let mut ours = false;
    for dir in missing.into_iter().rev() {
        ours = ours || dir.file_name().is_some_and(|n| n == "freesynergy") || dir == path;
        make_dir(dir, ours).or_else(|e| {
            // another process made it between the look and the making
            if e.kind() == io::ErrorKind::AlreadyExists && dir.is_dir() {
                Ok(())
            } else {
                Err(named(dir, e))
            }
        })?;
    }
    Ok(())
}

#[cfg(unix)]
fn make_dir(dir: &Path, private: bool) -> io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    let mut builder = std::fs::DirBuilder::new();
    if private {
        builder.mode(0o700);
    }
    builder.create(dir)
}

#[cfg(not(unix))]
fn make_dir(dir: &Path, _private: bool) -> io::Result<()> {
    std::fs::create_dir(dir)
}

#[cfg(all(test, unix))]
#[path = "datadir_tests.rs"]
mod tests;
