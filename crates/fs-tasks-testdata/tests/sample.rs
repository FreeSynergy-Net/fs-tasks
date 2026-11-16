//! The fixed sample: its size, its cases and its invented people.

use fs_tasks_testdata::tasks_v1::{
    AssigneeFilter, AssigneeStatus, Due, Order, Page, Progress, Readiness, Restorable, TaskQuery,
    Tasks, Trash, Zone,
};
use fs_tasks_testdata::{KNOWN, occurrence, sample};

fn query(zone: &str) -> TaskQuery {
    TaskQuery {
        lists: Vec::new(),
        text: None,
        progress: Vec::new(),
        due: None,
        undated: None,
        assignee: None,
        labels: Vec::new(),
        parent: None,
        ready: None,
        order: Order::Rank,
        view_zone: Zone::iana(zone).unwrap(),
        trash: Trash::Exclude,
    }
}

async fn all(fake: &impl Tasks) -> Page<fs_tasks_testdata::tasks_v1::TaskSummary> {
    fake.list(&query("Europe/Berlin"), None, 1000)
        .await
        .unwrap()
}

#[test]
fn the_sample_has_about_ninety_tasks_in_six_lists_with_unique_ids() {
    let s = sample();
    assert_eq!(s.lists.len(), 6);
    assert!(
        (80..=100).contains(&s.tasks.len()),
        "{} tasks",
        s.tasks.len()
    );
    let mut ids: Vec<_> = s.tasks.iter().map(|(_, t)| t.id).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), s.tasks.len());
    let mut uids: Vec<_> = s.tasks.iter().map(|(_, t)| t.uid.clone()).collect();
    uids.sort();
    uids.dedup();
    assert_eq!(uids.len(), s.tasks.len());
    for l in &s.lists {
        assert!(
            s.tasks.iter().any(|(_, t)| t.list == l.id),
            "list {} is empty",
            l.name
        );
    }
}

#[test]
fn the_people_are_invented_and_live_at_example_org() {
    for k in KNOWN {
        assert!(k.address.ends_with("@example.org"), "{}", k.address);
    }
    assert_eq!(KNOWN.len(), 9);
}

#[tokio::test]
async fn the_sample_covers_the_due_forms_priorities_and_answers() {
    let s = sample();
    let all: Vec<_> = s.tasks.iter().map(|(_, t)| t).collect();
    assert!(all.iter().any(|t| matches!(t.due, Some(Due::Date(_)))));
    assert!(all.iter().any(|t| matches!(t.due, Some(Due::Timed { .. }))));
    assert!(all.iter().any(|t| matches!(t.due, Some(Due::Floating(_)))));
    assert!(all.iter().any(|t| t.due.is_none() && t.start.is_none()));
    assert!(
        (0..=5).all(|p| all.iter().any(|t| t.priority == p)
            || p == 4 && all.iter().any(|t| t.priority == 4))
    );
    let answers: Vec<_> = all
        .iter()
        .flat_map(|t| t.assignees.iter().map(|a| a.status))
        .collect();
    for want in [
        AssigneeStatus::NeedsAction,
        AssigneeStatus::Accepted,
        AssigneeStatus::Declined,
    ] {
        assert!(answers.contains(&want));
    }
    for p in [
        Progress::NeedsAction,
        Progress::InProcess,
        Progress::Completed,
        Progress::Failed,
        Progress::Cancelled,
    ] {
        assert!(all.iter().any(|t| t.progress == p), "{p:?}");
    }
    assert!(all.iter().any(|t| !t.recurrence.is_empty()));
    assert!(all.iter().any(|t| t.parent.is_some()));
    assert!(all.iter().any(|t| t.claim.is_some()));
}

#[tokio::test]
async fn the_loaded_fake_shows_every_readiness_and_the_series() {
    let s = sample();
    let fake = s.fake();
    let page = all(&fake).await;
    let by = |slug: &str| {
        page.items
            .iter()
            .find(|x| x.id == s.get(slug).id)
            .unwrap()
            .clone()
    };
    assert_eq!(by("strom").readiness, Readiness::Ready);
    assert!(matches!(
        by("zaehlerstand").readiness,
        Readiness::Blocked(_)
    ));
    assert!(matches!(
        by("schluessel").readiness,
        Readiness::DependencyCancelled(_)
    ));
    assert!(matches!(by("sofa").readiness, Readiness::Unknown(_)));
    assert!(matches!(by("checkliste").readiness, Readiness::Conflict(_)));
    assert!(
        matches!(by("kontakt").readiness, Readiness::Ready)
            && matches!(by("bilder").readiness, Readiness::Blocked(_))
    );
    assert_eq!(by("mietvertrag").readiness, Readiness::Done);
    let pflanzen = by("pflanzen");
    assert_eq!(pflanzen.occurrence, occurrence("2026-11-17T00:00:00"));
    assert!(pflanzen.recurring);
    assert_eq!(by("einpacken").subtasks_total, 3);
    assert_eq!(by("einpacken").subtasks_open, 2);
    assert!(by("kasse").claimed_by.is_some());
    assert_eq!(by("stammtisch").occurrence, None);
}

#[tokio::test]
async fn the_trash_holds_three_tasks_and_the_logbook_is_ordered() {
    let s = sample();
    let fake = s.fake();
    let mut q = query("Europe/Berlin");
    q.trash = Trash::Only;
    let gone = fake.list(&q, None, 10).await.unwrap();
    assert_eq!(gone.items.len(), 3);
    let state = |id| gone.items.iter().find(|t| t.id == id).and_then(|t| t.trash);
    assert!(matches!(state(s.trashed.1.id), Some(Restorable::Until(_))));
    assert_eq!(state(s.manual.1.id), Some(Restorable::Manual));
    assert_eq!(state(s.held.1.id), Some(Restorable::Held(None)));
    let mut q = query("Europe/Berlin");
    q.progress = vec![Progress::Completed];
    q.order = Order::Finished;
    let book = fake.list(&q, None, 100).await.unwrap().items;
    assert!(book.len() >= 7);
    assert_eq!(book[0].title, "Brot kaufen");
}

#[tokio::test]
async fn the_tokyo_task_counts_for_the_17th_in_berlin_and_the_16th_in_utc() {
    use fs_tasks_testdata::tasks_v1::{DateField, DueRange};
    use fs_types::BasicValue;
    let s = sample();
    let fake = s.fake();
    let day = fs_tasks_testdata::tasks_v1::Date::parse("2026-11-17").unwrap();
    let range = |zone: &str| {
        let mut q = query(zone);
        q.due = Some(DueRange {
            field: DateField::Due,
            from: Some(day),
            to: Some(day),
        });
        q
    };
    let berlin = fake
        .list(&range("Europe/Berlin"), None, 100)
        .await
        .unwrap()
        .items;
    let utc = fake.list(&range("UTC"), None, 100).await.unwrap().items;
    let id = s.get("partnerbuero").id;
    assert!(berlin.iter().any(|x| x.id == id));
    assert!(!utc.iter().any(|x| x.id == id));
}

#[tokio::test]
async fn the_delta_sample_has_group_lists_group_tasks_checklists_and_series_from_completion() {
    use fs_tasks_testdata::tasks_v1::{ListAccess, RepeatFrom};
    let s = sample();
    let fake = s.fake();
    let lists = fake.lists(Trash::Exclude).await.unwrap();
    let board = lists
        .iter()
        .find(|l| l.name == "Vorstand Beispielhausen")
        .unwrap();
    assert_eq!(board.owner, KNOWN[5].person());
    assert_eq!(board.access, ListAccess::Manage); // Anna manages "Team Vorstand"
    // The asking person (Anna) is in "Team Vorstand" (6), not in "Team Umzug" (7).
    let mut q = query("Europe/Berlin");
    q.assignee = Some(AssigneeFilter::MyGroups);
    let mine: Vec<_> = fake.list(&q, None, 100).await.unwrap().items;
    let slugs: Vec<_> = mine.iter().map(|t| t.id).collect();
    assert!(slugs.contains(&s.get("jahresversammlung").id));
    assert!(slugs.contains(&s.get("kassenpruefung").id));
    assert!(!slugs.contains(&s.get("umzugsteam").id));
    let shopping = fake.get(s.get("wocheneinkauf").id).await.unwrap().unwrap();
    assert_eq!(
        shopping
            .checklist
            .iter()
            .filter(|c| c.done.is_none())
            .count(),
        2
    );
    assert_eq!(shopping.checklist.len(), 5);
    for slug in ["blumen", "rauchmelder", "wasserfilter"] {
        assert_eq!(s.get(slug).repeat_from, RepeatFrom::Completion, "{slug}");
    }
    let flowers = fake.get(s.get("blumen").id).await.unwrap().unwrap();
    assert_eq!(flowers.overrides.len(), 2);
    assert_eq!(
        s.get("kassenpruefung").claim.as_ref().map(|c| c.by.clone()),
        Some(KNOWN[1].person())
    );
}

#[tokio::test]
async fn the_group_the_asker_is_not_in_cannot_claim_and_a_member_can() {
    let s = sample();
    let fake = s.fake();
    let team = s.get("umzugsteam");
    let denied = fake
        .claim(
            team.id,
            team.revision,
            fs_tasks_testdata::tasks_v1::Utc::from_millis(fake.now().millis() + 60_000),
        )
        .await;
    assert!(matches!(
        denied,
        Err(fs_tasks_testdata::tasks_v1::TaskError::Common(
            fs_tasks_testdata::tasks_v1::Error::Forbidden { .. }
        ))
    ));
    let board = s.get("jahresversammlung");
    let ok = fake
        .claim(
            board.id,
            board.revision,
            fs_tasks_testdata::tasks_v1::Utc::from_millis(fake.now().millis() + 60_000),
        )
        .await
        .unwrap();
    assert_eq!(ok.claim.unwrap().by, fake.account());
}
