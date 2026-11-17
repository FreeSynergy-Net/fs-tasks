//! The server as a command: what `fs-tasks serve` runs.
//!
//! Pattern: Command (one subcommand of the single `fs-tasks` program, ADR 0063) — reads its options,
//! opens the store, serves until a signal. Returns the exit code; the binary exits with it.

use std::ffi::OsString;
use std::sync::Arc;

use fs_interfaces::rpc::{
    ServerIdentity, ServerOptions, default_data_dir, prepare_data_dir, run_until_signal_as,
};
use fs_objects::{Store, TursoStore};
use fs_tasks_core::TaskRepo;

use crate::TasksServer;
use crate::datadir_start::correct;
use crate::datadir_text::{Lang, fault_text, tightened_text};

const USAGE: &str = "fs-tasks serve [--socket PATH] [--data DIR] [--actor IRI] [--instance DOMAIN]\nDefaults: $XDG_RUNTIME_DIR/freesynergy/tasks.sock, $XDG_DATA_HOME/freesynergy/tasks, the local owner";

/// Run the server with `args` (the words after `serve`); the exit code: 0 after a signal or `--help`,
/// 2 for a mistake in the arguments, 1 when serving failed.
#[must_use]
pub fn serve_command(args: impl IntoIterator<Item = OsString>) -> i32 {
    let options = match ServerOptions::parse("tasks", args) {
        Ok(Some(options)) => options,
        Ok(None) => {
            println!("{USAGE}");
            return 0;
        }
        Err(problem) => {
            eprintln!("fs-tasks serve: {problem}");
            return 2;
        }
    };
    let ServerOptions {
        socket,
        data,
        actor,
        instance,
        facet,
    } = options;
    // Tasks has one view only; a facet name is refused at the start, so a wrong link is found early.
    if let Some(facet) = facet {
        eprintln!("fs-tasks serve: no facet named {facet:?} (Tasks has none)");
        return 2;
    }
    // Name and version go to `Control.version`: a window of another version replaces this server (fs-interfaces handshake).
    let identity = ServerIdentity::new("fs-tasks", env!("CARGO_PKG_VERSION"));
    let served = run_until_signal_as(&socket, identity, || {
        crate::early::until_signal(async move {
            correct_data_dir(&data)?;
            let data = prepare_data_dir(&data)?;
            let store = TursoStore::open(data.join("objects.db"))
                .await
                .map_err(std::io::Error::other)?;
            let store: Arc<dyn Store> = Arc::new(store);
            let mut repo = TaskRepo::new(store, actor);
            if let Some(instance) = instance {
                repo = repo.with_instance(instance);
            }
            // Tasks stored before 0.7.0 get the index fields the pages need; what cannot be repaired is
            // named, and a page says so as well.
            let report = repo.upgrade_index().await.map_err(std::io::Error::other)?;
            if report.rewritten > 0 {
                eprintln!(
                    "fs-tasks serve: index completed for {} tasks",
                    report.rewritten
                );
            }
            for failed in &report.failed {
                eprintln!("fs-tasks serve: task not repaired: {failed}");
            }
            let repo = Arc::new(repo);
            Ok(TasksServer::new(repo.clone(), repo).into_client().client)
        })
    });
    match served {
        Ok(()) => 0,
        // A stop signal during the start is a stop, not a failure.
        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {
            eprintln!("fs-tasks serve: {e}");
            0
        }
        Err(e) => {
            eprintln!("fs-tasks serve: {e}");
            1
        }
    }
}

/// Take the bits away from an open data folder of this user before the transport looks at it (it refuses anything but
/// 0700). What was done is said on stderr (the log of the server); a refused folder ends the start with its code and
/// parameters, then the sentence in the language of the person.
fn correct_data_dir(data: &std::path::Path) -> std::io::Result<()> {
    let lang = Lang::from_env();
    match correct(data, default_data_dir("tasks").as_deref()) {
        Ok(Some(done)) => {
            eprintln!(
                "fs-tasks serve: {}\n{}",
                done.params(),
                tightened_text(&done, lang)
            );
            Ok(())
        }
        Ok(None) => Ok(()),
        Err(fault) => Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!("{}\n{}", fault.params(), fault_text(&fault, lang)),
        )),
    }
}
