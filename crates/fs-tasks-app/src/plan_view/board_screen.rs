//! Screen A: the board as the plan widgets draw it, over the picture of the store.
//!
//! Pattern: Adapter — the screen is a `Surface`; its state (the chosen card, the column being added to, the label
//! shown) lives here, the widgets only draw and report. Reading is from the cache, so drawing never waits for the
//! core; creating goes through the adapter's form and renews the picture.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use fs_components_app::{Element, Surface};
use fs_components_widgets::plan::{BoardView, Column, Entry, Events, SideItem, Tab};
use fs_components_workbenches::tasks::TaskSource;
use fs_tasks_core::validate::PROGRESS;
use fs_tasks_server::Lang;

use super::cache::Cache;
use super::cards::{card, panel};
use super::defaults::View;
use super::enter::{change_failed, create_task};
use super::notice::{Notice, notice_text};
use super::snapshot::Snapshot;
use crate::source::{TaskAdapter, label_list};
use crate::words::progress_title;
use fs_tasks_core::Day;

#[derive(Default)]
struct State {
    selected: Option<String>,
    creating: Option<String>,
    label: Option<String>,
    notice: Option<Notice>,
}

/// The columns of the board over `snapshot`: the cards of `label` (all when `None`), by `progress`.
#[must_use]
pub fn board_columns(
    snapshot: &Snapshot,
    label: Option<&str>,
    selected: Option<&str>,
) -> Vec<Column> {
    let mut columns: Vec<Column> = PROGRESS
        .iter()
        .map(|p| Column::new(p, progress_title(p)))
        .collect();
    for e in &snapshot.entries {
        if label.is_some_and(|l| !label_list(&e.task).iter().any(|x| x == l)) {
            continue;
        }
        let id = e.id.to_string();
        let at = PROGRESS
            .iter()
            .position(|p| *p == e.task.effective_progress());
        if let Some(i) = at {
            columns[i]
                .cards
                .push(card(snapshot, e, selected == Some(id.as_str())));
        }
    }
    columns
}

/// The board screen.
pub struct BoardScreen {
    adapter: Arc<TaskAdapter>,
    cache: Arc<Cache>,
    state: Arc<Mutex<State>>,
    entry: Entry,
    events: Events,
    lang: Lang,
}

fn held(s: &Mutex<State>) -> std::sync::MutexGuard<'_, State> {
    s.lock().unwrap_or_else(PoisonError::into_inner)
}

impl BoardScreen {
    /// The board over `cache`; creating a task goes through `adapter`.
    pub fn new(adapter: Arc<TaskAdapter>, cache: Arc<Cache>, mode: Arc<super::mode::Mode>) -> Self {
        let state = Arc::new(Mutex::new(State::default()));
        let mut events = Events::default();
        let s = Arc::clone(&state);
        events.pick = Some(Arc::new(move |key| {
            let mut st = held(&s);
            if let Some(label) = key.strip_prefix("label:") {
                st.label = Some(label.to_string());
            } else if key == "all" {
                st.label = None;
            } else {
                st.selected = (st.selected.as_deref() != Some(key)).then(|| key.to_string());
            }
        }));
        events.tab = Some(Arc::new(move |key| {
            if key == "list" {
                mode.set_list(true);
            }
        }));
        let (a, c, st) = (Arc::clone(&adapter), Arc::clone(&cache), Arc::clone(&state));
        events.act = Some(Arc::new(move |key, to| {
            let Some(entry) = c.current().and_then(|p| p.entry(key).cloned()) else {
                return;
            };
            let moved = a.move_to(key, to, crate::source::row_of(&entry).revision.as_deref());
            held(&st).notice = moved.as_ref().err().map(change_failed);
            if moved.is_ok() {
                c.changed();
            }
        }));
        let s = Arc::clone(&state);
        events.close = Some(Arc::new(move || held(&s).selected = None));
        let s = Arc::clone(&state);
        events.create = Some(Arc::new(move |col| {
            held(&s).creating = Some(col.to_string());
        }));
        let (a, c, st) = (Arc::clone(&adapter), Arc::clone(&cache), Arc::clone(&state));
        events.enter = Some(Arc::new(move |col, text| {
            enter_in(&a, &c, &st, col, text);
        }));
        Self {
            adapter,
            cache,
            state,
            entry: Entry::new("What needs to be done?"),
            events,
            lang: Lang::from_env(),
        }
    }

    /// Typed `text` in the entry line of the column `column` (what Enter does).
    pub fn submit(&self, column: &str, text: &str) {
        enter_in(&self.adapter, &self.cache, &self.state, column, text);
    }

    /// Choose the project (label) shown on the left; `None` shows all tasks.
    pub fn show_label(&self, label: Option<&str>) {
        held(&self.state).label = label.map(str::to_string);
    }

    /// The message shown now, as a sentence in the language of the person, if there is one.
    #[must_use]
    pub fn notice(&self) -> Option<String> {
        held(&self.state)
            .notice
            .as_ref()
            .map(|n| notice_text(n, self.lang))
    }

    /// The message shown now as data (code and parameters).
    #[must_use]
    pub fn notice_data(&self) -> Option<Notice> {
        held(&self.state).notice.clone()
    }

    /// Choose the task whose title holds `part` (for a picture of the open panel).
    pub fn select_title(&self, part: &str) {
        let part = part.to_lowercase();
        let found = self.cache.current().and_then(|s| {
            s.entries
                .iter()
                .find(|e| e.task.display_title().to_lowercase().contains(&part))
                .map(|e| e.id.to_string())
        });
        held(&self.state).selected = found;
    }

    fn projects(snapshot: &Snapshot, shown: Option<&str>) -> Vec<SideItem> {
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for e in &snapshot.entries {
            for l in label_list(&e.task) {
                *counts.entry(l).or_default() += 1;
            }
        }
        let mut items = vec![
            SideItem::heading("Projects (labels)"),
            SideItem::new("all", "All tasks", Some(snapshot.entries.len())),
        ];
        items.extend(
            counts
                .into_iter()
                .map(|(l, n)| SideItem::new(&format!("label:{l}"), &l, Some(n))),
        );
        for i in &mut items {
            i.selected = match shown {
                None => i.key == "all",
                Some(l) => i.key == format!("label:{l}"),
            };
        }
        items
    }
}

impl Surface for BoardScreen {
    fn build(&self, width: f32, height: f32) -> Box<dyn Element> {
        let Some(snapshot) = self.cache.current() else {
            let why = self
                .cache
                .problem()
                .unwrap_or_else(|| "Reading the tasks and what blocks them ...".into());
            return Box::new(fs_components_widgets::plan::message(&why, width, height));
        };
        let (selected, creating, label) = {
            let st = held(&self.state);
            (st.selected.clone(), st.creating.clone(), st.label.clone())
        };
        let columns = board_columns(&snapshot, label.as_deref(), selected.as_deref());
        let panel = selected
            .as_deref()
            .and_then(|k| snapshot.entry(k))
            .map(|e| panel(&snapshot, e));
        let tabs = [
            Tab::new("board", "Board"),
            Tab::new("list", "List"),
            Tab::new("calendar", "Calendar").locked(),
            Tab::new("timeline", "Timeline").locked(),
        ];
        let mut projects = Self::projects(&snapshot, label.as_deref());
        if let Some(n) = held(&self.state).notice.as_ref() {
            projects.insert(0, SideItem::heading(&notice_text(n, self.lang)));
        }
        let view = BoardView {
            tabs: &tabs,
            active: "board",
            soon: "Not built yet",
            projects: &projects,
            columns: &columns,
            panel: panel.as_ref(),
            creating: creating.as_deref(),
            entry: &self.entry,
            create: "Create",
            close: "Close",
            events: &self.events,
        };
        Box::new(view.build("fs-plan-board", width, height))
    }

    fn styles(&self) -> String {
        fs_components_widgets::styles()
    }

    fn status_line(&self) -> bool {
        false
    }
}

/// Enter in the entry line of `column`: the task takes what the view gives it; a refusal or a task the view does not
/// show becomes the message of the screen.
fn enter_in(a: &TaskAdapter, c: &Arc<Cache>, st: &Mutex<State>, column: &str, text: &str) {
    let label = held(st).label.clone();
    let view = View::Board {
        label: label.as_deref(),
        column,
    };
    let today = c.current().map_or_else(Day::today, |p| p.today);
    let outcome = create_task(a, &view, today, text);
    let ok = outcome.is_ok();
    held(st).notice = outcome.unwrap_or_else(Some);
    if ok {
        c.changed();
    }
}
