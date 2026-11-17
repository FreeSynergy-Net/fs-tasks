//! Screen B: the day list over the picture of the store, and the surface that holds both screens.
//!
//! Pattern: Adapter (a `Surface`) + Facade (`PlanSurface` picks the screen by the shared mode). State lives here:
//! the list chosen, the lines opened; the widgets only draw and report. Ticking a box finishes (or reopens) the
//! task through the adapter; Enter in the entry line makes a task (with what the list gives it) and renews the
//! picture; a refusal or a task the list does not show is told in the note of the list.

use std::sync::{Arc, Mutex, PoisonError};

use fs_components_app::{Element, Surface};
use fs_components_widgets::plan::{Entry, Events, ListScreen};
use fs_components_workbenches::tasks::TaskSource;
use fs_tasks_server::Lang;

use super::board_screen::BoardScreen;
use super::cache::Cache;
use super::defaults::View;
use super::enter::{change_failed, create_task};
use super::mode::Mode;
use super::next_data::{heading, sections, side};
use super::notice::{Notice, notice_text};
use crate::source::{TaskAdapter, row_of};
use fs_tasks_core::Day;

struct State {
    chosen: String,
    opened: Vec<String>,
    notice: Option<Notice>,
}

/// The day list.
pub struct NextScreen {
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

impl NextScreen {
    /// The list over `cache`; changes go through `adapter`; `mode` is shared with the board.
    pub fn new(adapter: Arc<TaskAdapter>, cache: Arc<Cache>, mode: Arc<Mode>) -> Self {
        let state = Arc::new(Mutex::new(State {
            chosen: "today".into(),
            opened: Vec::new(),
            notice: None,
        }));
        let mut events = Events::default();
        let (s, m) = (Arc::clone(&state), mode);
        events.pick = Some(Arc::new(move |key| {
            if key == "view:board" {
                m.set_list(false);
            } else {
                let mut st = held(&s);
                st.chosen = key.to_string();
                st.notice = None;
            }
        }));
        let s = Arc::clone(&state);
        events.open = Some(Arc::new(move |key| {
            let mut st = held(&s);
            match st.opened.iter().position(|k| k == key) {
                Some(i) => {
                    st.opened.remove(i);
                }
                None => st.opened.push(key.to_string()),
            }
        }));
        let (a, c, st) = (Arc::clone(&adapter), Arc::clone(&cache), Arc::clone(&state));
        events.tick = Some(Arc::new(move |key| {
            let Some(entry) = c.current().and_then(|p| p.entry(key).cloned()) else {
                return;
            };
            let finished = !crate::source::is_open(&entry.task);
            let to = if finished {
                "needs-action"
            } else {
                "completed"
            };
            let moved = a.move_to(key, to, row_of(&entry).revision.as_deref());
            held(&st).notice = moved.as_ref().err().map(change_failed);
            if moved.is_ok() {
                c.changed();
            }
        }));
        let (a, c, s) = (Arc::clone(&adapter), Arc::clone(&cache), Arc::clone(&state));
        events.enter = Some(Arc::new(move |place, text| {
            enter_in(&a, &c, &s, place, text);
        }));
        Self {
            cache,
            state,
            entry: Entry::new("New to-do"),
            events,
            lang: Lang::from_env(),
            adapter,
        }
    }
}

/// Enter in the entry line of the list `place`: the task takes what the list gives it; a refusal or a task the list
/// does not show becomes the message of the screen.
fn enter_in(a: &TaskAdapter, c: &Arc<Cache>, st: &Mutex<State>, place: &str, text: &str) {
    let today = c.current().map_or_else(Day::today, |p| p.today);
    let outcome = create_task(a, &View::List { place }, today, text);
    let ok = outcome.is_ok();
    held(st).notice = outcome.unwrap_or_else(Some);
    if ok {
        c.changed();
    }
}

impl NextScreen {
    /// Open the line whose title holds `part` and show its list (for a picture of an open line).
    pub fn open_title(&self, part: &str) {
        let part = part.to_lowercase();
        let found = self.cache.current().and_then(|s| {
            s.entries
                .iter()
                .find(|e| e.task.display_title().to_lowercase().contains(&part))
                .map(|e| e.id.to_string())
        });
        if let Some(key) = found {
            held(&self.state).opened.push(key);
        }
    }

    /// Show the list with this key.
    pub fn show(&self, list: &str) {
        held(&self.state).chosen = list.to_string();
    }

    /// Typed `text` in the entry line of the list shown (what Enter does).
    pub fn submit(&self, text: &str) {
        let place = held(&self.state).chosen.clone();
        enter_in(&self.adapter, &self.cache, &self.state, &place, text);
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
}

impl Surface for NextScreen {
    fn build(&self, width: f32, height: f32) -> Box<dyn Element> {
        let Some(snapshot) = self.cache.current() else {
            let why = self
                .cache
                .problem()
                .unwrap_or_else(|| "Reading the tasks and what blocks them ...".into());
            return Box::new(fs_components_widgets::plan::message(&why, width, height));
        };
        let (chosen, opened) = {
            let st = held(&self.state);
            (st.chosen.clone(), st.opened.clone())
        };
        let (title, mut note) = heading(&chosen);
        if let Some(n) = held(&self.state).notice.as_ref() {
            note = format!("{}  {note}", notice_text(n, self.lang));
        }
        let (items, secs) = (
            side(&snapshot, &chosen),
            sections(&snapshot, &chosen, &opened),
        );
        let screen = ListScreen {
            side: &items,
            title: &title,
            note: Some(&note),
            sections: &secs,
            place: &chosen,
            entry: &self.entry,
            empty: "Nothing here.",
            events: &self.events,
        };
        Box::new(screen.build("fs-plan-list", width, height))
    }

    fn styles(&self) -> String {
        fs_components_widgets::styles()
    }

    fn status_line(&self) -> bool {
        false
    }
}

/// Both screens, one shown at a time.
pub struct PlanSurface {
    board: BoardScreen,
    list: NextScreen,
    mode: Arc<Mode>,
}

impl PlanSurface {
    /// The two screens over one cache; `list` says which one starts.
    pub fn new(adapter: Arc<TaskAdapter>, cache: Arc<Cache>, list: bool) -> Self {
        let mode = Arc::new(Mode::default());
        mode.set_list(list);
        Self {
            board: BoardScreen::new(Arc::clone(&adapter), Arc::clone(&cache), Arc::clone(&mode)),
            list: NextScreen::new(adapter, cache, Arc::clone(&mode)),
            mode,
        }
    }

    /// Choose the card of the board whose title holds `part` (a picture of the open panel).
    pub fn select_title(&self, part: &str) {
        if self.mode.is_list() {
            self.list.open_title(part);
        } else {
            self.board.select_title(part);
        }
    }

    /// Show the day list (`true`) or the board.
    pub fn set_list(&self, list: bool) {
        self.mode.set_list(list);
    }

    /// Show the day list `list` (for a picture).
    pub fn show_list(&self, list: &str) {
        self.list.show(list);
    }

    /// Show the project (label) of the board; `None` shows all tasks.
    pub fn show_label(&self, label: Option<&str>) {
        self.board.show_label(label);
    }

    /// Typed `text` in the entry line of the screen shown: of the list shown, or of the board column `column`.
    pub fn submit(&self, column: &str, text: &str) {
        if self.mode.is_list() {
            self.list.submit(text);
        } else {
            self.board.submit(column, text);
        }
    }

    /// The message of the screen shown, as a sentence.
    #[must_use]
    pub fn notice(&self) -> Option<String> {
        if self.mode.is_list() {
            self.list.notice()
        } else {
            self.board.notice()
        }
    }

    /// The message of the screen shown, as data (code and parameters).
    #[must_use]
    pub fn notice_data(&self) -> Option<Notice> {
        if self.mode.is_list() {
            self.list.notice_data()
        } else {
            self.board.notice_data()
        }
    }
}

impl Surface for PlanSurface {
    fn build(&self, width: f32, height: f32) -> Box<dyn Element> {
        if self.mode.is_list() {
            self.list.build(width, height)
        } else {
            self.board.build(width, height)
        }
    }

    fn styles(&self) -> String {
        fs_components_widgets::styles()
    }

    fn status_line(&self) -> bool {
        false
    }
}
