//! Tests of the clean-up after a take-over: only the socket file of the ended process goes.
//!
//! Pattern: Table tests on real files in a temporary folder — the file that belonged to the process, a successor's socket
//! (another file), and a file that is no socket.

use std::os::unix::fs::MetadataExt;
use std::os::unix::net::UnixListener;

use crate::ending::remove_if_same;

#[test]
fn only_the_socket_file_that_belonged_to_the_ended_process_is_removed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tasks.sock");
    let old = UnixListener::bind(&path).unwrap();
    let ino = std::fs::symlink_metadata(&path).unwrap().ino();
    // A successor made its own socket at the same path meanwhile: another file, it stays.
    std::fs::remove_file(&path).unwrap();
    let new = UnixListener::bind(&path).unwrap();
    assert!(
        !remove_if_same(&path, Some(ino)),
        "a successor's socket was removed"
    );
    assert!(path.exists());
    // The file that was seen (same inode) goes.
    let seen = std::fs::symlink_metadata(&path).unwrap().ino();
    assert!(remove_if_same(&path, Some(seen)));
    assert!(!path.exists());
    drop((old, new));
}

#[test]
fn nothing_is_removed_without_a_seen_file_or_when_it_is_no_socket() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tasks.sock");
    std::fs::write(&path, "not a socket").unwrap();
    let ino = std::fs::symlink_metadata(&path).unwrap().ino();
    assert!(
        !remove_if_same(&path, Some(ino)),
        "a plain file was removed"
    );
    assert!(!remove_if_same(&path, None));
    assert!(path.exists());
}
