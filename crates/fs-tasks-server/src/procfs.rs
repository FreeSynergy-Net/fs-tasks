//! What the system says about a process: start time, program file, user (Linux `/proc`).
//!
//! Pattern: Gateway — the only place that reads `/proc`; it makes the plain facts of `takeover` out of it. The
//! functions take the root of the tree, so the tests use a made-up one. Everything that cannot be read is `None`: the
//! rule then refuses, it never guesses.

use std::os::unix::fs::MetadataExt;
use std::path::Path;

use crate::takeover::{Image, Own, Peer};

/// The real `/proc`.
pub(crate) const PROC: &str = "/proc";

/// The state letter and the start time (ticks since boot) from the text of `/proc/<pid>/stat`. The name of the program
/// stands in brackets and may hold spaces and brackets itself, so the fields count from the last `)`.
pub(crate) fn parse_stat(text: &str) -> Option<(char, u64)> {
    let after = &text[text.rfind(')')? + 1..];
    let mut fields = after.split_whitespace();
    let state = fields.next()?.chars().next()?;
    // After the state: ppid pgrp session tty tpgid flags minflt cminflt majflt cmajflt utime stime cutime cstime
    // priority nice threads itrealvalue, then the start time (the 22nd field of the whole line).
    let start = fields.nth(18)?.parse().ok()?;
    Some((state, start))
}

/// The state and the start time of `pid`; `None` if there is no such process (or it is hidden).
pub(crate) fn stat_of(root: &Path, pid: i32) -> Option<(char, u64)> {
    parse_stat(&std::fs::read_to_string(root.join(pid.to_string()).join("stat")).ok()?)
}

/// The program file behind a link such as `/proc/<pid>/exe`: the file itself (the link is followed) and its real path.
pub(crate) fn image_of(link: &Path) -> Option<Image> {
    let meta = std::fs::metadata(link).ok()?;
    let path = std::fs::read_link(link).ok()?;
    let path = path.to_string_lossy();
    Some(Image {
        dev: meta.dev(),
        ino: meta.ino(),
        path: path.strip_suffix(" (deleted)").unwrap_or(&path).to_owned(),
    })
}

/// The facts about `pid`, found through the socket (`cred_uid` is what the socket said about its owner). The user is
/// the owner of `/proc/<pid>`; if that does not agree with the socket, the user counts as unknown.
pub(crate) fn peer_in(root: &Path, pid: i32, cred_uid: u32) -> Peer {
    let dir = root.join(pid.to_string());
    let owner = std::fs::metadata(&dir).ok().map(|m| m.uid());
    Peer {
        pid,
        uid: owner.filter(|o| *o == cred_uid).unwrap_or(u32::MAX),
        start: stat_of(root, pid).map(|(_, start)| start),
        exe: image_of(&dir.join("exe")),
    }
}

/// The facts about `pid` in the real `/proc`.
pub(crate) fn peer(pid: i32, cred_uid: u32) -> Peer {
    peer_in(Path::new(PROC), pid, cred_uid)
}

/// The facts about this process.
pub(crate) fn own() -> Option<Own> {
    Some(Own {
        pid: i32::try_from(std::process::id()).ok()?,
        uid: rustix::process::geteuid().as_raw(),
        exe: image_of(&Path::new(PROC).join("self").join("exe"))?,
    })
}

/// Whether the process is gone for good: no such process, a zombie (it holds nothing), or another process with the
/// same number (it started at another time than `start`).
pub(crate) fn is_gone(root: &Path, pid: i32, start: u64) -> bool {
    match stat_of(root, pid) {
        None => true,
        Some((state, now)) => state == 'Z' || state == 'X' || now != start,
    }
}

/// The seconds since the boot (`/proc/uptime`).
fn uptime(root: &Path) -> Option<f64> {
    std::fs::read_to_string(root.join("uptime"))
        .ok()?
        .split_whitespace()
        .next()?
        .parse()
        .ok()
}

/// How many seconds ago a process with this start time (ticks since boot) started.
pub(crate) fn age_secs(start: u64) -> Option<f64> {
    let ticks = rustix::param::clock_ticks_per_second();
    // Ticks stay far below 2^52 for any machine that runs long enough to matter, so the float is exact.
    #[allow(clippy::cast_precision_loss)]
    let began = start as f64 / ticks as f64;
    Some((uptime(Path::new(PROC))? - began).max(0.0))
}

/// The start of a process as UTC text (`2026-11-17T09:30:00Z`), for the message and the log.
pub(crate) fn started_utc(start: u64) -> Option<String> {
    let age = age_secs(start)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    // The age is below 2^52 seconds, the cast loses nothing a message needs.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let epoch = now.saturating_sub(age as u64);
    Some(utc(epoch))
}

/// Seconds since 1970 as `YYYY-MM-DDTHH:MM:SSZ` (the civil-date rule of H. Hinnant).
pub(crate) fn utc(epoch: u64) -> String {
    let (days, secs) = (epoch / 86_400, epoch % 86_400);
    let z = i64::try_from(days).unwrap_or(0) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        secs / 3600,
        secs % 3600 / 60,
        secs % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_start_time_is_the_22nd_field_after_the_last_bracket() {
        let line = "4242 (fs-tasks (x) y) S 1 4242 4242 0 -1 4194560 100 0 0 0 5 3 0 0 20 0 8 0 987654 1 2 3";
        assert_eq!(parse_stat(line), Some(('S', 987_654)));
        assert_eq!(parse_stat("garbage"), None);
        assert_eq!(parse_stat("1 (x) S 1 2"), None);
    }

    #[test]
    fn dates_come_out_right() {
        assert_eq!(utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(utc(1_000_000_000), "2001-09-09T01:46:40Z");
        assert_eq!(utc(1_763_380_800), "2025-11-17T12:00:00Z");
        assert_eq!(utc(951_782_400), "2000-02-29T00:00:00Z");
    }

    #[test]
    fn a_made_up_tree_gives_the_facts_and_a_zombie_or_a_new_start_is_gone() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("77");
        std::fs::create_dir(&dir).unwrap();
        let stat = |state: &str, start: u64| {
            format!("77 (x) {state} 1 77 77 0 -1 0 0 0 0 0 0 0 0 0 20 0 1 0 {start} 1")
        };
        std::fs::write(dir.join("stat"), stat("S", 500)).unwrap();
        let exe = std::env::current_exe().unwrap();
        std::os::unix::fs::symlink(&exe, dir.join("exe")).unwrap();
        let me = rustix::process::geteuid().as_raw();
        let found = peer_in(root.path(), 77, me);
        assert_eq!((found.pid, found.uid, found.start), (77, me, Some(500)));
        assert!(found.exe.unwrap().same_as(&own().unwrap().exe));
        // The socket named another user than the folder's owner: the user is unknown.
        assert_eq!(peer_in(root.path(), 77, me + 1).uid, u32::MAX);
        assert!(!is_gone(root.path(), 77, 500));
        assert!(
            is_gone(root.path(), 77, 501),
            "another start: another process"
        );
        std::fs::write(dir.join("stat"), stat("Z", 500)).unwrap();
        assert!(is_gone(root.path(), 77, 500), "a zombie holds nothing");
        assert!(is_gone(root.path(), 78, 500), "no such process");
    }
}
