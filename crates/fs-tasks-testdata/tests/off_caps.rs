//! A staged provider that switches a capability off must answer `Unsupported` when the method is called (Opus review
//! K1-1, A1). The mutant is a provider that shows the capability off and answers the call with a quiet `Ok`: the rule
//! must catch it, while the honest provider (the same fake, the call refused with `Unsupported`) stays green.

use std::sync::Arc;

use async_trait::async_trait;
use fs_tasks_testdata::conformance::{Options, run_conformance_with};
use fs_tasks_testdata::tasks_v1::{Capabilities, Error, TaskError, TaskResult};
use fs_tasks_testdata::{Decorated, FakeTasks, Gate, Op};

/// `task.assign`, `task.claim` and `relation.create` are switched off; `quiet` = the calls still work.
struct Dim {
    quiet: bool,
}

const OFF: [&str; 3] = ["task.assign", "task.claim", "relation.create"];

fn cap_of(op: Op) -> Option<&'static str> {
    match op {
        Op::Assign | Op::Unassign => Some("task.assign"),
        Op::Claim | Op::Release | Op::ClaimNext => Some("task.claim"),
        Op::Relate => Some("relation.create"),
        _ => None,
    }
}

#[async_trait]
impl Gate for Dim {
    async fn pass(&self, op: Op) -> TaskResult<()> {
        match cap_of(op) {
            Some(cap) if !self.quiet => Err(TaskError::Common(Error::Unsupported {
                capability: cap.to_owned(),
                reason: "switched off".to_owned(),
            })),
            _ => Ok(()),
        }
    }

    fn capabilities(&self, inner: Capabilities) -> Capabilities {
        Capabilities(
            inner
                .0
                .into_iter()
                .map(|(c, on)| {
                    let off = OFF.contains(&c.as_str());
                    (c, on && !off)
                })
                .collect(),
        )
    }
}

fn staged(quiet: bool) -> Decorated<Dim> {
    Decorated::with_gate(Arc::new(FakeTasks::new()), Dim { quiet })
}

fn partial() -> Options {
    Options {
        heavy: false,
        skip_rank: true,
        partial: true,
    }
}

#[tokio::test]
async fn an_honest_staged_provider_answers_unsupported_for_what_it_switched_off() {
    let failures = run_conformance_with(&staged(false), partial()).await;
    assert!(
        !failures.iter().any(|f| f.contains("switched-off")),
        "failed rules: {failures:#?}"
    );
}

#[tokio::test]
async fn a_quiet_ok_for_a_switched_off_capability_is_caught() {
    let failures = run_conformance_with(&staged(true), partial()).await;
    for method in [
        "assign",
        "unassign",
        "claim",
        "release",
        "claim_next",
        "relate",
    ] {
        assert!(
            failures
                .iter()
                .any(|f| f.starts_with(&format!("{method}: the switched-off capability"))),
            "a quiet Ok of `{method}` was not caught: {failures:#?}"
        );
    }
}
