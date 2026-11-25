//! The data folder: an open one (0755, as the window of 0.0.8 made it) is corrected and the program starts; a refused
//! one is named with its cause where the person sees it.

mod common;
use common::Limited;

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output};

fn fs_tasks(dir: &Path, args: &[&str]) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_fs-tasks"));
    c.env("XDG_RUNTIME_DIR", dir.join("run"))
        .env("XDG_DATA_HOME", dir.join("data"))
        .env("LC_ALL", "en_US.UTF-8")
        .env_remove("FS_TASKS_SOCKET")
        .args(args);
    c
}

fn run(dir: &Path, args: &[&str]) -> Output {
    fs_tasks(dir, args).output_limited()
}

fn mode(path: &Path) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

/// Ends the autostarted server of the test.
struct Cleanup<'a>(&'a Path);

impl Drop for Cleanup<'_> {
    fn drop(&mut self) {
        let socket = self.0.join("run/freesynergy/tasks.sock");
        let needle = socket.display().to_string();
        for entry in std::fs::read_dir("/proc").unwrap().flatten() {
            let Ok(raw) = std::fs::read(entry.path().join("cmdline")) else {
                continue;
            };
            if String::from_utf8_lossy(&raw)
                .replace('\0', " ")
                .contains(&needle)
            {
                let _ = Command::new("kill")
                    .args(["-TERM", &entry.file_name().to_string_lossy()])
                    .status();
            }
        }
    }
}

fn make(path: &Path, bits: u32) {
    std::fs::create_dir_all(path).unwrap();
    std::fs::set_permissions(path, PermissionsExt::from_mode(bits)).unwrap();
}

#[test]
fn a_data_folder_made_0755_is_corrected_logged_and_the_program_starts() {
    let dir = tempfile::tempdir().unwrap();
    let _stop = Cleanup(dir.path());
    let data = dir.path().join("data/freesynergy/tasks");
    make(&data, 0o755);
    let out = run(dir.path(), &["add", "Job", "--json"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(mode(&data), 0o700);
    let log = dir.path().join("run/freesynergy/tasks.log");
    assert_eq!(mode(&log), 0o600, "the log is for this user alone");
    let text = std::fs::read_to_string(&log).unwrap();
    assert!(
        text.contains("data-dir-tightened") && text.contains("old_mode=0755"),
        "{text}"
    );
    assert!(text.contains("may have been read"), "{text}");
}

#[test]
fn a_private_data_folder_leaves_no_correction_in_the_log() {
    let dir = tempfile::tempdir().unwrap();
    let _stop = Cleanup(dir.path());
    make(&dir.path().join("data/freesynergy/tasks"), 0o700);
    assert!(run(dir.path(), &["add", "Job", "--json"]).status.success());
    let text = std::fs::read_to_string(dir.path().join("run/freesynergy/tasks.log")).unwrap();
    assert!(!text.contains("data-dir-tightened"), "{text}");
}

#[test]
fn a_server_that_cannot_start_is_explained_with_its_cause_not_only_a_hint() {
    let dir = tempfile::tempdir().unwrap();
    let real = dir.path().join("real");
    make(&real, 0o700);
    let data = dir.path().join("data/freesynergy");
    make(&data, 0o700);
    std::os::unix::fs::symlink(&real, data.join("tasks")).unwrap();
    let out = run(dir.path(), &["add", "Job", "--json"]);
    assert!(!out.status.success());
    let text =
        String::from_utf8_lossy(&out.stderr).into_owned() + &String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("data-dir-symlink"), "{text}");
    assert!(text.contains("symbolic link"), "{text}");
    assert!(!text.contains("run `fs-tasks serve` to see why"), "{text}");
    assert_eq!(mode(&real), 0o700);
}

#[test]
fn a_data_path_that_is_a_file_is_refused_and_left_as_it_is() {
    // fs-rpc (since 9cbce63c) tightens an open folder of this user by itself, also one named with `--data`; what it refuses is
    // a path that is no folder. The refusal ends the start with code 1 and touches nothing.
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("mine");
    std::fs::write(&file, b"not a folder").unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_fs-tasks"))
        .args(["serve", "--socket"])
        .arg(dir.path().join("x.sock"))
        .arg("--data")
        .arg(&file)
        .env("LC_ALL", "en_US.UTF-8")
        .output_limited();
    assert_eq!(
        status.status.code(),
        Some(1),
        "the transport refuses a file"
    );
    assert_eq!(std::fs::read(&file).unwrap(), b"not a folder");
}
