//! Tests of the data folder rule and its application to real folders (Unix).
//!
//! Pattern: Table tests — the pure rule on made-up facts, then the same cases on real folders in a temporary folder.

use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
use std::path::Path;

use super::*;

const ME: u32 = 1000;

fn meta(mode: u32) -> Meta {
    Meta {
        is_dir: true,
        is_symlink: false,
        uid: ME,
        mode: 0o040_000 | mode,
    }
}

fn mode_of(path: &Path) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o7777
}

fn dir_with(mode: u32) -> (tempfile::TempDir, PathBuf) {
    let base = tempfile::tempdir().unwrap();
    let dir = base.path().join("tasks");
    std::fs::create_dir(&dir).unwrap();
    std::fs::set_permissions(&dir, PermissionsExt::from_mode(mode)).unwrap();
    (base, dir)
}

#[test]
fn the_rule_takes_bits_away_from_an_open_folder_of_mine() {
    let p = Path::new("/d");
    assert_eq!(decide(p, &meta(0o700), ME), Action::Keep);
    assert_eq!(decide(p, &meta(0o500), ME), Action::Keep);
    for open in [0o755, 0o750, 0o777, 0o705, 0o770, 0o701] {
        assert_eq!(
            decide(p, &meta(open), ME),
            Action::Tighten(open & 0o700),
            "{open:o}"
        );
    }
    // Never more than the owner had: a folder without owner bits does not gain them.
    assert_eq!(decide(p, &meta(0o055), ME), Action::Tighten(0));
}

#[test]
fn the_rule_refuses_links_files_and_folders_of_others() {
    let p = Path::new("/d");
    let link = Meta {
        is_symlink: true,
        ..meta(0o700)
    };
    assert_eq!(
        decide(p, &link, ME),
        Action::Refuse(Fault::Symlink { path: p.into() })
    );
    let file = Meta {
        is_dir: false,
        ..meta(0o700)
    };
    assert_eq!(
        decide(p, &file, ME),
        Action::Refuse(Fault::NotDirectory { path: p.into() })
    );
    // Even an open folder of somebody else is refused, never changed.
    let other = Meta {
        uid: 0,
        ..meta(0o777)
    };
    assert_eq!(
        decide(p, &other, ME),
        Action::Refuse(Fault::ForeignOwner {
            path: p.into(),
            owner: 0,
            me: ME
        })
    );
}

#[test]
fn an_open_folder_becomes_private_and_the_old_mode_is_reported() {
    for open in [0o755, 0o750, 0o777, 0o770, 0o705] {
        let (_base, dir) = dir_with(open);
        let done = tighten(&dir).unwrap().expect("it was open");
        assert_eq!(
            done,
            Tightened {
                path: dir.clone(),
                old_mode: open | 0o040_000
            }
        );
        assert_eq!(mode_of(&dir), 0o700, "{open:o}");
        assert!(
            done.params().contains(&format!("old_mode={open:04o}")),
            "{}",
            done.params()
        );
    }
}

#[test]
fn a_private_folder_is_left_alone_and_nothing_is_reported() {
    let (_base, dir) = dir_with(0o700);
    let before = std::fs::metadata(&dir).unwrap().ctime_nsec();
    assert_eq!(tighten(&dir), Ok(None));
    assert_eq!(mode_of(&dir), 0o700);
    assert_eq!(
        std::fs::metadata(&dir).unwrap().ctime_nsec(),
        before,
        "not even touched"
    );
    // A folder that is not there yet is for the transport to make.
    assert_eq!(tighten(&dir.join("none")), Ok(None));
}

#[test]
fn what_is_inside_is_not_changed() {
    let (_base, dir) = dir_with(0o755);
    let file = dir.join("objects.db");
    std::fs::write(&file, "x").unwrap();
    std::fs::set_permissions(&file, PermissionsExt::from_mode(0o644)).unwrap();
    tighten(&dir).unwrap();
    assert_eq!(mode_of(&file), 0o644);
}

#[test]
fn a_link_is_refused_and_its_target_stays_as_it_is() {
    let (base, target) = dir_with(0o755);
    let link = base.path().join("link");
    symlink(&target, &link).unwrap();
    let fault = tighten(&link).unwrap_err();
    assert_eq!(fault, Fault::Symlink { path: link });
    assert_eq!(fault.code(), "data-dir-symlink");
    assert_eq!(mode_of(&target), 0o755, "the target was not touched");
}

#[test]
fn a_file_is_refused() {
    let base = tempfile::tempdir().unwrap();
    let file = base.path().join("tasks");
    std::fs::write(&file, "x").unwrap();
    std::fs::set_permissions(&file, PermissionsExt::from_mode(0o644)).unwrap();
    assert_eq!(
        tighten(&file),
        Err(Fault::NotDirectory { path: file.clone() })
    );
    assert_eq!(mode_of(&file), 0o644);
}

#[test]
fn a_folder_of_somebody_else_is_refused_and_not_changed() {
    // `/usr` belongs to root (not to a user who runs the tests; as root there is nobody else to test with).
    let usr = Path::new("/usr");
    if rustix::process::geteuid().as_raw() == 0
        || std::fs::metadata(usr).map_or(1, |m| m.uid()) != 0
    {
        return;
    }
    let before = mode_of(usr);
    assert!(matches!(
        tighten(usr),
        Err(Fault::ForeignOwner { owner: 0, .. }) | Ok(None)
    ));
    assert_eq!(mode_of(usr), before);
}

#[test]
fn the_texts_exist_in_both_languages_for_every_fault() {
    use crate::datadir_text::{Lang, fault_text, tightened_text};
    let p = std::path::PathBuf::from("/d");
    let faults = [
        Fault::Symlink { path: p.clone() },
        Fault::NotDirectory { path: p.clone() },
        Fault::ForeignOwner {
            path: p.clone(),
            owner: 0,
            me: 1,
        },
        Fault::Io {
            path: p.clone(),
            kind: io::ErrorKind::NotFound,
        },
        Fault::StillOpen {
            path: p.clone(),
            mode: 0o755,
        },
    ];
    for fault in &faults {
        let (en, de) = (fault_text(fault, Lang::En), fault_text(fault, Lang::De));
        assert!(
            en.contains("/d") && de.contains("/d") && en != de,
            "{fault:?}"
        );
        assert!(fault.params().starts_with(fault.code()));
    }
    let done = Tightened {
        path: p,
        old_mode: 0o755,
    };
    assert!(tightened_text(&done, Lang::En).contains("0755"));
    assert!(tightened_text(&done, Lang::De).contains("war für andere offen"));
    assert_eq!(Lang::from_locale("de_DE.UTF-8"), Lang::De);
    assert_eq!(Lang::from_locale("C"), Lang::En);
}

#[test]
fn private_folders_are_made_0700_from_freesynergy_down_and_not_above() {
    let base = tempfile::tempdir().unwrap();
    let share = base.path().join("share");
    let leaf = share.join("freesynergy").join("tasks");
    create_private_dir(&leaf).unwrap();
    assert_eq!(mode_of(&leaf), 0o700);
    assert_eq!(mode_of(leaf.parent().unwrap()), 0o700);
    let plain = base.path().join("plain");
    std::fs::create_dir(&plain).unwrap();
    assert_eq!(
        mode_of(&share),
        mode_of(&plain),
        "the folder above gets the ordinary mode"
    );
    // An existing folder is not changed; making it again is no error.
    std::fs::set_permissions(&leaf, PermissionsExt::from_mode(0o755)).unwrap();
    create_private_dir(&leaf).unwrap();
    assert_eq!(mode_of(&leaf), 0o755);
}

#[test]
fn opening_never_follows_a_link_and_wants_a_folder() {
    // This is what makes the correction safe against a swap between the look and the change (O_NOFOLLOW, O_DIRECTORY).
    let (base, target) = dir_with(0o755);
    let link = base.path().join("link");
    symlink(&target, &link).unwrap();
    assert!(
        open_dir_nofollow(&link).is_err(),
        "a link must not be opened"
    );
    assert!(open_dir_nofollow(&target).is_ok());
    let file = base.path().join("file");
    std::fs::write(&file, "x").unwrap();
    assert!(open_dir_nofollow(&file).is_err(), "a file is no folder");
}

#[test]
fn only_the_standard_data_folder_is_corrected_not_one_named_with_data() {
    use crate::datadir_start::correct;
    let (_base, dir) = dir_with(0o755);
    let elsewhere = PathBuf::from("/nowhere/standard");
    // A folder named by hand (`--data ~/mine`, `--data ~`) is left as it is: the transport refuses it if it is open.
    assert_eq!(correct(&dir, Some(&elsewhere)), Ok(None));
    assert_eq!(correct(&dir, None), Ok(None));
    assert_eq!(mode_of(&dir), 0o755);
    // The standard one is corrected.
    assert!(correct(&dir, Some(&dir)).unwrap().is_some());
    assert_eq!(mode_of(&dir), 0o700);
}

#[test]
fn nothing_is_changed_below_a_path_that_others_can_write() {
    use crate::datadir_start::correct;
    let base = tempfile::tempdir().unwrap();
    let above = base.path().join("freesynergy");
    let dir = above.join("tasks");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::set_permissions(&dir, PermissionsExt::from_mode(0o755)).unwrap();
    std::fs::set_permissions(&above, PermissionsExt::from_mode(0o777)).unwrap();
    let fault = correct(&dir, Some(&dir)).unwrap_err();
    assert_eq!(
        fault,
        Fault::Ancestor {
            path: dir.clone(),
            ancestor: above.canonicalize().unwrap()
        }
    );
    assert_eq!(fault.code(), "data-dir-ancestor");
    assert_eq!(
        mode_of(&dir),
        0o755,
        "the folder was not touched before the path was checked"
    );
    // Safe again: now it is corrected.
    std::fs::set_permissions(&above, PermissionsExt::from_mode(0o700)).unwrap();
    assert!(correct(&dir, Some(&dir)).unwrap().is_some());
    assert_eq!(mode_of(&dir), 0o700);
}
