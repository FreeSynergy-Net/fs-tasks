//! Where the window's tasks come from: the server (the motor) over the socket, or a store of its own.
//!
//! Pattern: Strategy — one motor per device holds the data (`fs-tasks serve`); the window is a client of it, like the
//! command line and the agents, and starts it if none runs. Only an explicit `--store` file, a picture or an export
//! without `--connect` keep the data in the process (memory or that file), so they touch nothing else.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use fs_objects::{OpenOptions, Store, TursoStore};
use fs_rpc::default_socket;
use fs_tasks_core::{TaskItems, TaskRepo};
use fs_tasks_server::{Changes, RemoteItems, StartError};

use crate::args::Args;

/// Who acts on a store the window holds itself. Provisional: there is no sign-in yet.
const ACTOR: &str = "https://localhost/actors/owner";

/// The store the window, `fs-tasks serve` and the command line share: the file the server opens by default.
const SHARED_STORE: &str = "objects.db";

/// Why the tasks could not be opened: the server holds the socket and does not answer (or the start cannot proceed), which
/// the window shows with its own words, or anything else, in a text.
#[derive(Debug)]
pub enum OpenError {
    /// The server could not be reached (code and parameters in the error).
    Server(StartError),
    /// Anything else.
    Text(String),
}

impl From<String> for OpenError {
    fn from(text: String) -> Self {
        OpenError::Text(text)
    }
}

impl From<&str> for OpenError {
    fn from(text: &str) -> Self {
        OpenError::Text(text.to_owned())
    }
}

impl From<StartError> for OpenError {
    fn from(error: StartError) -> Self {
        OpenError::Server(error)
    }
}

impl std::fmt::Display for OpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OpenError::Server(error) => write!(f, "tasks server: {error}"),
            OpenError::Text(text) => f.write_str(text),
        }
    }
}

/// The tasks and, for a window that is a client, the change notices of the server.
pub struct Opened {
    /// The provider the window reads and writes.
    pub items: Arc<dyn TaskItems>,
    /// Tells when anybody changed the tasks; `None` for a store held in the process.
    pub changes: Option<Arc<Changes>>,
}

/// Whether the window is a client of the server: with `--connect`, or by default (no `--store`, no picture, no export).
fn is_client(args: &Args) -> bool {
    args.connect.is_some()
        || (args.store.is_none() && args.snapshot.is_none() && args.export.is_none())
}

/// The data folder of the program: `$XDG_DATA_HOME/freesynergy/tasks`.
pub(crate) fn data_dir() -> Option<PathBuf> {
    crate::frontend::data_dir_in(std::env::var_os("XDG_DATA_HOME"), std::env::var_os("HOME"))
}

/// Up to 0.0.7 the window kept its tasks in `app.db`; the server keeps them in `objects.db`. A window that finds
/// only the old file moves it (with its side files) to the shared name, so nothing is lost and nothing is split.
fn adopt_legacy_store(dir: &Path) -> Result<(), String> {
    if dir.join(SHARED_STORE).exists() || !dir.join("app.db").exists() {
        return Ok(());
    }
    let named = |e: std::io::Error| format!("{}: {e}", dir.display());
    for entry in std::fs::read_dir(dir).map_err(named)? {
        let from = entry.map_err(named)?.path();
        let name = from.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if let Some(rest) = name.strip_prefix("app.db") {
            std::fs::rename(&from, dir.join(format!("{SHARED_STORE}{rest}"))).map_err(named)?;
        }
    }
    Ok(())
}

/// Make the data folder before the server runs. It is made 0700 (never 0755: the transport of the server refuses any
/// other mode, and the program did not start up to 0.0.8); a folder that exists is left to the server, which takes the
/// open bits away from a folder of its own.
fn prepare_own_dir(dir: &Path) -> Result<(), String> {
    fs_tasks_server::create_private_dir(dir)?;
    adopt_legacy_store(dir)
}

/// Connect to the server (starting it if none answers). The window never opens the data file in this case.
fn client(args: &Args) -> Result<Opened, OpenError> {
    let socket = args
        .connect
        .clone()
        .or_else(|| default_socket("tasks"))
        .ok_or("no socket: give --connect <socket> or set XDG_RUNTIME_DIR")?;
    if args.connect.is_none()
        && let Some(dir) = data_dir()
    {
        prepare_own_dir(&dir)?;
    }
    let remote = RemoteItems::connect(socket, None)?;
    let changes = remote.changes();
    Ok(Opened {
        items: Arc::new(remote),
        changes: Some(changes),
    })
}

/// A store in this process: `--store`, or memory (a picture, an export).
fn own(args: &Args, runtime: &tokio::runtime::Runtime) -> Result<Opened, OpenError> {
    let store: Arc<dyn Store> = runtime
        .block_on(async {
            let owner = fs_tasks_core::store_owner(ACTOR)
                .map_err(|e| fs_objects::Error::Invalid(e.to_string()))?;
            let options = OpenOptions {
                allow_sensitive_unencrypted: args.store.is_none(),
                ..OpenOptions::default()
            }
            .with_owner(owner);
            match &args.store {
                Some(file) => TursoStore::open_with(file, options).await,
                None => TursoStore::open_with(":memory:", options).await,
            }
        })
        .map(|s| Arc::new(s) as Arc<dyn Store>)
        .map_err(|e| {
            let hint = if e.to_string().contains("locked") {
                " (another fs-tasks holds it, e.g. `fs-tasks serve`: use --connect <socket> to use that one)"
            } else {
                ""
            };
            format!("cannot open the store: {e}{hint}")
        })?;
    let repo = Arc::new(TaskRepo::new(store, ACTOR));
    // Tasks stored by 0.0.4 or before have no dependency fields in their index (and 0.6.0 ones no order key): complete
    // them as the server does, so the window sees the same readiness; what cannot be repaired is named.
    let report = runtime
        .block_on(repo.upgrade_index())
        .map_err(|e| format!("cannot complete the index: {e}"))?;
    if report.rewritten > 0 {
        eprintln!("fs-tasks: index completed for {} tasks", report.rewritten);
    }
    for failed in &report.failed {
        eprintln!("fs-tasks: task not repaired: {failed}");
    }
    Ok(Opened {
        items: repo,
        changes: None,
    })
}

/// The tasks for `args`.
///
/// # Errors
/// The text says why the server or the store could not be reached.
pub fn open(args: &Args, runtime: &tokio::runtime::Runtime) -> Result<Opened, OpenError> {
    if is_client(args) {
        client(args)
    } else {
        own(args, runtime)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_old_app_db_moves_to_the_shared_name_with_its_side_files_and_never_over_a_new_one() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["app.db", "app.db-wal", "settings.json"] {
            std::fs::write(dir.path().join(name), name).unwrap();
        }
        adopt_legacy_store(dir.path()).unwrap();
        let read = |n: &str| std::fs::read_to_string(dir.path().join(n)).ok();
        assert_eq!(read("objects.db").as_deref(), Some("app.db"));
        assert_eq!(read("objects.db-wal").as_deref(), Some("app.db-wal"));
        assert!(read("app.db").is_none() && read("settings.json").is_some());
        // With a shared store in place an old file is left alone: nothing is overwritten.
        std::fs::write(dir.path().join("app.db"), "old").unwrap();
        adopt_legacy_store(dir.path()).unwrap();
        assert_eq!(read("objects.db").as_deref(), Some("app.db"));
        assert_eq!(read("app.db").as_deref(), Some("old"));
    }

    #[cfg(unix)]
    #[test]
    fn the_window_makes_the_data_folder_private_so_the_server_starts() {
        use std::os::unix::fs::PermissionsExt;
        let base = tempfile::tempdir().unwrap();
        let dir = base.path().join("share/freesynergy/tasks");
        prepare_own_dir(&dir).unwrap();
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&dir), 0o700, "the window never makes it 0755");
        assert_eq!(
            mode(dir.parent().unwrap()),
            0o700,
            "nor the folder of the suite"
        );
        // What the server then does to it: nothing to correct, so it starts.
        assert_eq!(fs_tasks_server::tighten(&dir), Ok(None));
    }

    #[test]
    fn the_window_is_a_client_unless_it_keeps_a_store_of_its_own() {
        let plain = Args::default();
        assert!(is_client(&plain));
        let with = |f: fn(&mut Args)| {
            let mut a = Args::default();
            f(&mut a);
            a
        };
        assert!(!is_client(&with(|a| a.store = Some("x.db".into()))));
        assert!(!is_client(&with(|a| a.snapshot = Some("x.png".into()))));
        assert!(!is_client(&with(|a| a.export = Some("x.ics".into()))));
        // `--connect` wins: a picture of what the server holds.
        assert!(is_client(&with(|a| {
            a.snapshot = Some("x.png".into());
            a.connect = Some("s.sock".into());
        })));
    }
}
