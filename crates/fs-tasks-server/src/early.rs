//! A stop signal that works while the server is still opening its store.
//!
//! Pattern: Decorator — wraps the work of `open` so that SIGTERM or Ctrl-C ends it. `fs-interfaces` (`serve.rs`) binds
//! the socket first, installs its handlers, and only then runs `open`; a signal that comes meanwhile is remembered and
//! acts after `open` is done, so a server stuck in `open` could not be stopped (kill -9 was the only way). With this the
//! signal ends the waiting work at the next `await`.

use std::future::Future;
use std::io;

/// Run `work`; end it with an `Interrupted` error if SIGTERM or Ctrl-C comes first.
///
/// # Errors
/// What `work` returns, or `Interrupted` after a signal.
pub(crate) async fn until_signal<T>(work: impl Future<Output = io::Result<T>>) -> io::Result<T> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut term = signal(SignalKind::terminate())?;
        let mut interrupt = signal(SignalKind::interrupt())?;
        tokio::select! {
            done = work => done,
            _ = term.recv() => Err(stopped()),
            _ = interrupt.recv() => Err(stopped()),
        }
    }
    #[cfg(not(unix))]
    {
        work.await
    }
}

#[cfg(unix)]
fn stopped() -> io::Error {
    io::Error::new(
        io::ErrorKind::Interrupted,
        "stopped by a signal before the store was open",
    )
}

#[cfg(all(test, unix))]
mod tests {
    use std::time::Duration;

    use super::*;

    #[tokio::test]
    async fn a_stop_signal_ends_work_that_is_still_waiting_and_finished_work_is_returned() {
        // Work that is done first is returned as it is.
        assert_eq!(until_signal(async { Ok(7) }).await.unwrap(), 7);
        // Work that never ends is ended by SIGTERM: the handlers are in place when the future is first polled, so the
        // signal sent a moment later does not kill this process.
        let sender = std::thread::spawn(|| {
            std::thread::sleep(Duration::from_millis(400));
            let me =
                rustix::process::Pid::from_raw(i32::try_from(std::process::id()).unwrap()).unwrap();
            rustix::process::kill_process(me, rustix::process::Signal::TERM).unwrap();
        });
        let ended = tokio::time::timeout(
            Duration::from_secs(10),
            until_signal(std::future::pending::<io::Result<()>>()),
        )
        .await
        .expect("the signal did not end the waiting work");
        sender.join().unwrap();
        assert_eq!(ended.unwrap_err().kind(), io::ErrorKind::Interrupted);
    }
}
