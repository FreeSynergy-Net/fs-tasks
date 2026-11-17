//! The correction at the start of the server: only the standard data folder, only below a safe path.
//!
//! Pattern: Guard — the folders above the data folder are checked first (as the transport does: a folder of this user
//! or of root that others cannot write, or a sticky one like `/tmp`); only then is the folder itself made private, and
//! only if it is the standard one (`$XDG_DATA_HOME/freesynergy/tasks`). A folder named with `--data` is never changed:
//! `--data ~` must not close the home folder of the person. Such a folder that is open is refused by the transport.

use std::path::Path;

use crate::datadir::{Fault, Tightened, tighten};

/// The folders above `path` (checked on the real path) must be safe. A folder that is not there yet is for the
/// transport to make: nothing to check.
///
/// # Errors
/// [`Fault::Ancestor`] with the first open folder above.
#[cfg(unix)]
pub fn check_ancestors(path: &Path) -> Result<(), Fault> {
    use std::os::unix::fs::MetadataExt;

    let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) else {
        return Ok(());
    };
    let Ok(real) = parent.canonicalize() else {
        return Ok(());
    };
    let me = rustix::process::geteuid().as_raw();
    for ancestor in real.ancestors() {
        let Ok(meta) = std::fs::metadata(ancestor) else {
            continue;
        };
        let foreign = meta.uid() != me && meta.uid() != 0;
        if foreign || (meta.mode() & 0o022 != 0 && meta.mode() & 0o1000 == 0) {
            return Err(Fault::Ancestor {
                path: path.to_path_buf(),
                ancestor: ancestor.to_path_buf(),
            });
        }
    }
    Ok(())
}

/// No modes outside Unix.
///
/// # Errors
/// Never.
#[cfg(not(unix))]
pub fn check_ancestors(_path: &Path) -> Result<(), Fault> {
    Ok(())
}

/// What the server does with its data folder before the transport sees it: check the path above, and make the folder
/// private if it is the `standard` one (`None`: there is none). `Ok(None)`: nothing was changed.
///
/// # Errors
/// The fault says why the folder is refused or could not be corrected.
pub fn correct(data: &Path, standard: Option<&Path>) -> Result<Option<Tightened>, Fault> {
    check_ancestors(data)?;
    if standard == Some(data) {
        tighten(data)
    } else {
        Ok(None)
    }
}
