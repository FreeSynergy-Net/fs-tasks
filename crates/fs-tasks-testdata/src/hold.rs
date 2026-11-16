//! `HeldTasks`: a provider whose calls stay open until `release` (the loading state Z2 that a frontend can SHOW).
//!
//! Pattern: Decorator over `dyn Tasks` with a gate that is a latch (closed: every call waits, open: calls pass). No sleep,
//! no timer, no runtime: the waiting call is a future that is woken by `release`, so a test drives it with any executor
//! (poll it, see it pending, check `waiting()`, release, poll again).

use crate::decorate::{Decorated, Gate, Op};
use crate::tasks_v1::{TaskResult, Tasks};
use async_trait::async_trait;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::{Context, Poll, Waker};

#[derive(Default)]
struct Latch {
    closed: bool,
    waiting: usize,
    wakers: Vec<Waker>,
}

/// The gate of [`HeldTasks`].
#[derive(Default)]
pub struct HoldGate {
    latch: Mutex<Latch>,
}

fn locked(m: &Mutex<Latch>) -> MutexGuard<'_, Latch> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// One waiting call (counted while it waits, also when it is dropped before the gate opens).
struct Wait<'a> {
    gate: &'a HoldGate,
    counted: bool,
}

impl Future for Wait<'_> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let gate = self.gate;
        let mut latch = locked(&gate.latch);
        if !latch.closed {
            if self.counted {
                latch.waiting -= 1;
                drop(latch);
                self.counted = false;
            }
            return Poll::Ready(());
        }
        if self.counted {
            latch.wakers.push(cx.waker().clone());
        } else {
            latch.waiting += 1;
            latch.wakers.push(cx.waker().clone());
            drop(latch);
            self.counted = true;
        }
        Poll::Pending
    }
}

impl Drop for Wait<'_> {
    fn drop(&mut self) {
        if self.counted {
            locked(&self.gate.latch).waiting -= 1;
        }
    }
}

#[async_trait]
impl Gate for HoldGate {
    async fn pass(&self, _op: Op) -> TaskResult<()> {
        Wait {
            gate: self,
            counted: false,
        }
        .await;
        Ok(())
    }
}

/// A provider whose calls wait while the gate is closed ([`HeldTasks::hold`]) and go on at [`HeldTasks::release`].
pub type HeldTasks = Decorated<HoldGate>;

impl HeldTasks {
    /// Wrap `inner`; the gate is open (nothing waits yet).
    #[must_use]
    pub fn new(inner: Arc<dyn Tasks>) -> Self {
        Self {
            inner,
            gate: HoldGate::default(),
        }
    }

    /// Close the gate: every call from now on stays open.
    pub fn hold(&self) {
        locked(&self.gate.latch).closed = true;
    }

    /// Open the gate and wake every waiting call.
    pub fn release(&self) {
        let wakers = {
            let mut latch = locked(&self.gate.latch);
            latch.closed = false;
            std::mem::take(&mut latch.wakers)
        };
        for w in wakers {
            w.wake();
        }
    }

    /// How many calls wait right now.
    #[must_use]
    pub fn waiting(&self) -> usize {
        locked(&self.gate.latch).waiting
    }
}
