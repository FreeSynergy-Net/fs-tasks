//! The window that says the server does not answer, and asks whether to end it.
//!
//! Pattern: Strategy — a `Surface` of the shell that draws one reason instead of the tasks, and two menu entries as the
//! two answers (the shell has no question dialog yet): "Yes: end it and start again" runs the take-over (the same safety
//! rules as at the start), "No: close". The status line says the problem as well. Nothing here is grey without a sentence.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use fs_components_app::{
    AppError, AppWidget, Element, Menu, MenuEntry, Screen, Status, Surface, WindowOptions, launch,
};
use fs_components_widgets::{Block, Inline, TextView};
use fs_tasks_server::{Lang, StartError, Stuck, end_confirmed, start_text, window_words};

const YES: &str = "server-end";
const NO: &str = "server-close";

/// What the person decided.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The silent server was ended: start again.
    Retry,
    /// The window was closed without ending it.
    Closed,
}

/// Ends the silent server (the person said yes); the failure is shown in the status line.
pub type End = Arc<dyn Fn() -> Result<(), StartError> + Send + Sync>;

struct ProblemSurface {
    blocks: Vec<Block>,
}

impl Surface for ProblemSurface {
    fn build(&self, _width: f32, _height: f32) -> Box<dyn Element> {
        Box::new(TextView::new(self.blocks.clone()).build())
    }

    fn styles(&self) -> String {
        fs_components_widgets::styles()
    }
}

fn blocks(error: &StartError, lang: Lang) -> Vec<Block> {
    let (title, _, _) = window_words(lang);
    let mut blocks = vec![
        Block::Heading {
            level: 2,
            text: Inline::plain(title),
        },
        Block::Paragraph(Inline::plain(start_text(error, lang))),
    ];
    let params = error.params();
    blocks.push(Block::Paragraph(Inline::plain(format!(
        "{} {params}",
        error.code()
    ))));
    blocks
}

/// The window of `error`: the text, the status line, and the two answers. `retry` is set when the server was ended.
///
/// # Errors
/// The shell cannot be built.
pub fn build(
    error: &StartError,
    lang: Lang,
    end: End,
    retry: Arc<AtomicBool>,
) -> Result<AppWidget, AppError> {
    let (title, yes, no) = window_words(lang);
    let surface = Arc::new(ProblemSurface {
        blocks: blocks(error, lang),
    });
    let app = AppWidget::new(
        title,
        vec![Screen::new("problem", "", surface as Arc<dyn Surface>)],
    )?;
    app.set_status(Some(Status::Problem(start_text(error, lang))));
    let mut yes_entry = MenuEntry::new(YES, yes);
    if !error.stuck().is_some_and(|s| s.can_ask) {
        yes_entry = yes_entry.disabled();
    }
    let me = app.clone();
    app.set_menus(
        vec![Menu::new(title, vec![yes_entry, MenuEntry::new(NO, no)])],
        move |id| match id {
            YES => match end() {
                Ok(()) => {
                    retry.store(true, Ordering::SeqCst);
                    me.quit();
                }
                Err(failed) => {
                    eprintln!("fs-tasks: {}", failed.report(lang));
                    me.set_status(Some(Status::Problem(start_text(&failed, lang))));
                }
            },
            _ => {
                me.quit();
            }
        },
    )?;
    Ok(app)
}

/// Open the window and return when it is closed.
///
/// # Errors
/// The window cannot be opened (the reason is already said on stderr by the caller).
pub fn show(error: &StartError, lang: Lang, socket: &std::path::Path) -> Result<Outcome, String> {
    let stuck: Option<Stuck> = error.stuck().cloned();
    let socket = socket.to_path_buf();
    let end: End = Arc::new(move || match &stuck {
        Some(stuck) => end_confirmed(&socket, stuck),
        None => Err(StartError::Failed("nothing to end".into())),
    });
    let retry = Arc::new(AtomicBool::new(false));
    let app = build(error, lang, end, Arc::clone(&retry)).map_err(|e| e.to_string())?;
    launch(WindowOptions::default(), app).map_err(|e| e.to_string())?;
    Ok(if retry.load(Ordering::SeqCst) {
        Outcome::Retry
    } else {
        Outcome::Closed
    })
}
