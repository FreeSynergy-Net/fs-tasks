//! The log of a server the client started: what it says when it cannot start.
//!
//! Pattern: Value object around one file — `<socket>.log`, mode 0600, one per program, overwritten at each start,
//! at most 64 KB (only the end is kept). The start error of the client shows the last lines, so the person sees the
//! cause instead of "run it by hand".

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// The most the file keeps.
pub const MAX_BYTES: u64 = 64 * 1024;

/// How many lines the start error shows.
const SHOWN_LINES: usize = 10;

/// The log file for the server at `socket`.
#[must_use]
pub fn path_for(socket: &Path) -> PathBuf {
    socket.with_extension("log")
}

/// Open the log for a new server: made if missing, emptied if there, mode 0600 in every case.
///
/// # Errors
/// The system error.
pub fn open_fresh(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.create(true).write(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let file = options.open(path)?;
    #[cfg(unix)]
    file.set_permissions(std::os::unix::fs::PermissionsExt::from_mode(0o600))?;
    Ok(file)
}

/// Cut the file to its last `max` bytes, if it is larger (the cut falls in a line at most once, at the start).
///
/// # Errors
/// The system error.
pub fn keep_tail(path: &Path, max: u64) -> std::io::Result<()> {
    let size = std::fs::metadata(path)?.len();
    if size <= max {
        return Ok(());
    }
    let mut tail = Vec::new();
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(size - max))?;
    file.read_to_end(&mut tail)?;
    open_fresh(path)?.write_all(&tail)
}

/// The last lines of the log, or `None` if there is nothing to show.
#[must_use]
pub fn last_lines(path: &Path) -> Option<String> {
    let _ = keep_tail(path, MAX_BYTES);
    let text = String::from_utf8_lossy(&std::fs::read(path).ok()?).into_owned();
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let from = lines.len().saturating_sub(SHOWN_LINES);
    let shown = lines[from..].join("\n");
    (!shown.is_empty()).then_some(shown)
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    #[test]
    fn the_log_is_private_overwritten_and_cut_to_its_end() {
        let dir = tempfile::tempdir().unwrap();
        let log = path_for(&dir.path().join("tasks.sock"));
        assert_eq!(log.file_name().unwrap(), "tasks.log");
        // An old log with open rights is emptied and made 0600.
        std::fs::write(&log, "old").unwrap();
        std::fs::set_permissions(&log, PermissionsExt::from_mode(0o644)).unwrap();
        open_fresh(&log).unwrap();
        assert_eq!(
            std::fs::metadata(&log).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(std::fs::read(&log).unwrap(), b"");
        assert_eq!(last_lines(&log), None);
        // More than the limit: only the end stays; the start error shows ten lines.
        let text: String = (0..20_000)
            .map(|i| ["line ", &i.to_string(), "\n"].concat())
            .collect();
        std::fs::write(&log, &text).unwrap();
        let shown = last_lines(&log).unwrap();
        assert_eq!(shown.lines().count(), 10);
        assert!(shown.ends_with("line 19999"), "{shown}");
        assert!(std::fs::metadata(&log).unwrap().len() <= MAX_BYTES);
    }
}
