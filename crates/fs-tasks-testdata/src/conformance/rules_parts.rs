//! Rules for the alerts and the links of a task: what is written is read back whole and in order, `None` leaves them, an empty
//! list clears them, the card counts them, and a map with a key twice or a link that is neither an address nor a file is
//! `Invalid`.
//!
//! Pattern: Template method (rules as plain checks collected in the report).

use super::cx::{Cx, is_invalid};
use crate::tasks_v1::{
    Alert, AlertAction, FileRef, Link, MediaType, RelativeTo, SeriesScope, TaskPatch, Trigger, Utc,
};
use fs_types::{BasicValue, Reference};

fn alert(key: &str, trigger: Trigger, action: AlertAction, ack: Option<i64>) -> Alert {
    Alert {
        key: key.to_owned(),
        trigger,
        action,
        acknowledged: ack.map(Utc::from_millis),
    }
}

fn link(key: &str, href: Option<&str>, file: Option<FileRef>) -> Link {
    Link {
        key: key.to_owned(),
        href: href.map(str::to_owned),
        file,
        content_type: Some(MediaType("text/plain".to_owned())),
        size: Some(12),
        rel: Some("related".to_owned()),
        title: Some("A title".to_owned()),
    }
}

fn with(alerts: Option<Vec<Alert>>, links: Option<Vec<Link>>) -> TaskPatch {
    TaskPatch {
        alerts,
        links,
        ..TaskPatch::default()
    }
}

pub(super) async fn rules(cx: &mut Cx<'_>) {
    let dir = cx.dir;
    let Some(list) = cx.list("parts").await else {
        return;
    };
    let Ok(iri) = Reference::parse(
        "urn:sha256:0000000000000000000000000000000000000000000000000000000000000001",
    ) else {
        cx.ensure(false, "setup: a file reference");
        return;
    };
    let file = FileRef {
        iri,
        sha256: [7; 32],
        media_type: MediaType("image/png".to_owned()),
    };
    let alerts = vec![
        alert(
            "b",
            Trigger::Offset {
                seconds: -900,
                relative_to: RelativeTo::End,
            },
            AlertAction::Display,
            None,
        ),
        alert(
            "a",
            Trigger::Absolute {
                at: Utc::from_millis(1_800_000_000_000),
            },
            AlertAction::Email,
            Some(1_800_000_100_000),
        ),
    ];
    let links = vec![
        link("two", Some("https://example.org/x"), None),
        link("one", None, Some(file)),
    ];
    let Some(t) = cx
        .task_with(
            list.id,
            "parts",
            with(Some(alerts.clone()), Some(links.clone())),
        )
        .await
    else {
        return;
    };
    let read = cx.fresh(t.id).await;
    cx.ensure(
        read.as_ref().is_some_and(|r| r.alerts == alerts),
        "alerts: written whole, read back equal and in the order given",
    );
    cx.ensure(
        read.as_ref().is_some_and(|r| r.links == links),
        "links: an address and a file of ours, written whole, read back equal and in the order given",
    );
    let card = cx.summary(list.id, t.id).await;
    cx.ensure(
        card.is_some_and(|c| c.alerts == 2 && c.links == 2),
        "list: the card counts the alerts and the links",
    );
    let Ok(same) = dir
        .update(t.id, t.revision, TaskPatch::default(), SeriesScope::Whole)
        .await
    else {
        cx.ensure(false, "update: an empty patch");
        return;
    };
    cx.ensure(
        same.alerts == alerts && same.links == links,
        "update: None leaves the alerts and the links",
    );
    let Ok(one) = dir
        .update(
            t.id,
            same.revision,
            with(Some(vec![alerts[1].clone()]), None),
            SeriesScope::Whole,
        )
        .await
    else {
        cx.ensure(false, "update: replace the alerts");
        return;
    };
    cx.ensure(
        one.alerts == [alerts[1].clone()] && one.links == links,
        "update: Some replaces the whole set of alerts and leaves the links",
    );
    let Ok(cleared) = dir
        .update(
            t.id,
            one.revision,
            with(Some(Vec::new()), Some(Vec::new())),
            SeriesScope::Whole,
        )
        .await
    else {
        cx.ensure(false, "update: clear");
        return;
    };
    cx.ensure(
        cleared.alerts.is_empty() && cleared.links.is_empty(),
        "update: an empty list clears the alerts and the links",
    );
    let bad = |a: Option<Vec<Alert>>, l: Option<Vec<Link>>| with(a, l);
    let r = dir
        .update(
            t.id,
            cleared.revision,
            bad(Some(vec![alerts[0].clone(), alerts[0].clone()]), None),
            SeriesScope::Whole,
        )
        .await;
    cx.fails(&r, "alerts: a key twice is Invalid", is_invalid);
    let r = dir
        .update(
            t.id,
            cleared.revision,
            bad(None, Some(vec![link("k", None, None)])),
            SeriesScope::Whole,
        )
        .await;
    cx.fails(
        &r,
        "links: neither an address nor a file is Invalid",
        is_invalid,
    );
    let both = link("k", Some("https://example.org"), links[1].file.clone());
    let r = dir
        .update(
            t.id,
            cleared.revision,
            bad(None, Some(vec![both])),
            SeriesScope::Whole,
        )
        .await;
    cx.fails(
        &r,
        "links: an address and a file together is Invalid",
        is_invalid,
    );
    let r = dir
        .update(
            t.id,
            cleared.revision,
            bad(None, Some(vec![links[0].clone(), links[0].clone()])),
            SeriesScope::Whole,
        )
        .await;
    cx.fails(&r, "links: a key twice is Invalid", is_invalid);
    let r = dir
        .update(
            t.id,
            cleared.revision,
            bad(
                Some(vec![alert(
                    "",
                    alerts[0].trigger,
                    AlertAction::Display,
                    None,
                )]),
                None,
            ),
            SeriesScope::Whole,
        )
        .await;
    cx.fails(&r, "alerts: an empty key is Invalid", is_invalid);
    let still = cx.fresh(t.id).await;
    cx.ensure(
        still.is_some_and(|s| s.revision == cleared.revision),
        "update: a refused patch changes nothing (no new revision)",
    );
}
