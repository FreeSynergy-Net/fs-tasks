//! Time limits for the tests that run the real program: a hang fails the test after 60 s instead of the whole run.
//!
//! Pattern: Decorator — `Limited` adds `output_limited`, `status_limited` and `wait_limited` to `Command` and `Child`; the
//! child that overruns is killed (our own child, by its handle) and the test panics with what hung.

#![allow(dead_code)] // each test file uses a different subset

use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::time::{Duration, Instant};

/// The most a run of the real program may take in a test.
pub const LIMIT: Duration = Duration::from_secs(60);

/// Wait for `child` at most `LIMIT`; kill it and panic if it does not end.
pub fn wait_limited(child: &mut Child) -> ExitStatus {
    let began = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        if began.elapsed() > LIMIT {
            let _ = child.kill();
            let _ = child.wait();
            panic!("the program did not end within {LIMIT:?}: it hangs");
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// `Command::output` with the limit.
pub trait Limited {
    /// Run, collect the output, within `LIMIT`.
    fn output_limited(&mut self) -> Output;
    /// Run and return the status, within `LIMIT`.
    fn status_limited(&mut self) -> ExitStatus;
}

impl Limited for Command {
    fn output_limited(&mut self) -> Output {
        let mut child = self
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let (mut out, mut err) = (child.stdout.take().unwrap(), child.stderr.take().unwrap());
        let readers = [
            std::thread::spawn(move || {
                let mut v = Vec::new();
                std::io::Read::read_to_end(&mut out, &mut v).ok();
                v
            }),
            std::thread::spawn(move || {
                let mut v = Vec::new();
                std::io::Read::read_to_end(&mut err, &mut v).ok();
                v
            }),
        ];
        let status = wait_limited(&mut child);
        let [stdout, stderr] = readers.map(|r| r.join().unwrap());
        Output {
            status,
            stdout,
            stderr,
        }
    }

    fn status_limited(&mut self) -> ExitStatus {
        let mut child = self.spawn().unwrap();
        wait_limited(&mut child)
    }
}
