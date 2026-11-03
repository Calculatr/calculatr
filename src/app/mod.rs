mod clipboard;
mod input;
mod settings_view;

use std::io::{self, Stdout, stdout};
use std::time::{Duration, Instant};

use clipboard::Clipboard;
use crossterm::cursor::SetCursorStyle;
use crossterm::event::{self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyEventKind};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use crossterm::{ExecutableCommand, execute};
use ratatui::{Terminal, backend::CrosstermBackend};
use ratatui_image::picker::Picker;

use crate::features::core::{Environment, RenderMode};
use crate::features::editor::{Editor, ListNav, Mode};
use crate::features::history::HistoryEntry;
use crate::features::persistence::{self, SavedState};
use crate::features::settings::Settings;
use crate::features::tui::{self, images::ImageCache, theme::Theme};

pub use settings_view::{SettingsRow, SettingsView, settings_rows};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Calculator,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Input,
    History,
    Variables,
}

pub struct Status {
    pub text: String,
    pub is_error: bool,
    pub since: Instant,
}

const STATUS_LIFETIME: Duration = Duration::from_secs(5);

pub struct App {
    pub tab: Tab,
    pub focus: Focus,
    pub editor: Editor,
    pub history: Vec<HistoryEntry>,
    pub history_nav: ListNav,
    pub env: Environment,
    pub render_mode: RenderMode,
    pub variables_open: bool,
    pub var_nav: ListNav,
    pub var_filter: String,
    pub var_filter_active: bool,
    pub settings: Settings,
    pub settings_view: SettingsView,
    pub status: Option<Status>,
    pub theme: Theme,
    pub picker: Picker,
    pub images: ImageCache,
    pub should_quit: bool,
    pub(crate) clipboard: Clipboard,
    persist: bool,
    cursor_shape: Option<SetCursorStyle>,
}

impl App {
    pub fn new(picker: Picker) -> Self {
        let mut app = Self::build(
            picker,
            persistence::load_settings(),
            persistence::load_state(),
        );
        app.persist = true;
        app.clipboard = Clipboard::system();
        app
    }

    pub fn build(picker: Picker, settings: Settings, state: Option<SavedState>) -> Self {
        let (render_mode, history, env) = match state {
            Some(s) => (s.render_mode, s.history, s.env),
            None => (RenderMode::Decimal, Vec::new(), Environment::new()),
        };
        Self {
            tab: Tab::Calculator,
            focus: Focus::Input,
            editor: Editor::new(),
            history,
            history_nav: ListNav::default(),
            env,
            render_mode,
            variables_open: false,
            var_nav: ListNav::default(),
            var_filter: String::new(),
            var_filter_active: false,
            settings,
            settings_view: SettingsView::default(),
            status: None,
            theme: Theme::cyberpunk(),
            picker,
            images: ImageCache::new(),
            should_quit: false,
            clipboard: Clipboard::disabled(),
            persist: false,
            cursor_shape: None,
        }
    }

    pub fn set_status(&mut self, text: impl Into<String>) {
        self.status = Some(Status {
            text: text.into(),
            is_error: false,
            since: Instant::now(),
        });
    }

    pub fn set_error(&mut self, text: impl Into<String>) {
        self.status = Some(Status {
            text: text.into(),
            is_error: true,
            since: Instant::now(),
        });
    }

    fn expire_status(&mut self) -> bool {
        if self
            .status
            .as_ref()
            .is_some_and(|s| s.since.elapsed() > STATUS_LIFETIME)
        {
            self.status = None;
            return true;
        }
        false
    }

    pub fn current_mode_label(&self) -> &'static str {
        match self.tab {
            Tab::Settings => "SETTINGS",
            Tab::Calculator => match self.focus {
                Focus::Input => self.editor.mode().label(),
                Focus::History if self.history_nav.is_visual() => Mode::Visual.label(),
                Focus::Variables if self.var_filter_active => Mode::Insert.label(),
                Focus::Variables if self.var_nav.is_visual() => Mode::Visual.label(),
                _ => Mode::Normal.label(),
            },
        }
    }

    fn wanted_cursor_shape(&self) -> SetCursorStyle {
        let typing = self.tab == Tab::Calculator
            && ((self.focus == Focus::Input && self.editor.mode() == Mode::Insert)
                || (self.focus == Focus::Variables && self.var_filter_active));
        if typing {
            SetCursorStyle::SteadyBar
        } else {
            SetCursorStyle::SteadyBlock
        }
    }

    fn sync_cursor_shape(&mut self, out: &mut Stdout) -> io::Result<()> {
        let wanted = self.wanted_cursor_shape();
        if self.cursor_shape != Some(wanted) {
            execute!(out, wanted)?;
            self.cursor_shape = Some(wanted);
        }
        Ok(())
    }

    fn save(&self) {
        if self.persist {
            persistence::save_state(self.render_mode, &self.history, &self.env);
        }
    }
}

struct TerminalGuard;

impl TerminalGuard {
    fn new() -> io::Result<Self> {
        enable_raw_mode()?;
        let guard = Self;
        stdout().execute(EnterAlternateScreen)?;
        stdout().execute(EnableBracketedPaste)?;
        Ok(guard)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(stdout(), SetCursorStyle::DefaultUserShape);
        let _ = stdout().execute(LeaveAlternateScreen);
        let _ = stdout().execute(DisableBracketedPaste);
        let _ = disable_raw_mode();
    }
}

pub fn run() -> io::Result<()> {
    let _guard = TerminalGuard::new()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());
    let mut app = App::new(picker);

    let mut dirty = true;
    while !app.should_quit {
        if dirty {
            terminal.draw(|f| tui::draw(f, &mut app))?;
            app.sync_cursor_shape(&mut stdout())?;
            dirty = false;
        }

        if event::poll(Duration::from_millis(250))? {
            match event::read()? {
                Event::Key(key) if key.kind != KeyEventKind::Release => {
                    app.on_key(key);
                    dirty = true;
                }
                Event::Paste(text) => {
                    app.on_paste(&text);
                    dirty = true;
                }
                Event::Resize(..) => dirty = true,
                _ => {}
            }
        }
        dirty |= app.expire_status();
    }

    app.save();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{Terminal, backend::TestBackend};

    fn app() -> App {
        App::build(Picker::halfblocks(), Settings::default(), None)
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn type_text(app: &mut App, text: &str) {
        for c in text.chars() {
            app.on_key(key(KeyCode::Char(c)));
        }
    }

    fn input(app: &App) -> String {
        app.editor.buffer.text()
    }

    fn screen(app: &mut App, w: u16, h: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal
            .draw(|f| crate::features::tui::draw(f, app))
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        (0..h)
            .map(|y| {
                (0..w)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn frac_expands_and_tab_walks_the_parameters() {
        let mut app = app();
        type_text(&mut app, "frac");
        app.on_key(key(KeyCode::Tab));
        assert_eq!(input(&app), "frac(<>, <>)");
        type_text(&mut app, "1");
        app.on_key(key(KeyCode::Tab));
        type_text(&mut app, "4");
        app.on_key(key(KeyCode::Tab));
        assert_eq!(input(&app), "frac(1, 4)");
        assert_eq!(app.editor.buffer.cursor(), input(&app).chars().count());
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.history.last().unwrap().answer.text(), "= 0.25");
    }

    #[test]
    fn typing_closing_bracket_reuses_the_auto_closed_one() {
        let mut app = app();
        type_text(&mut app, "sin(1)");
        assert_eq!(input(&app), "sin(1)");
    }

    #[test]
    fn assignment_and_render_modes() {
        let mut app = app();
        type_text(&mut app, "x := 0.25");
        app.on_key(key(KeyCode::Enter));
        type_text(&mut app, "x");
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.history.last().unwrap().answer.text(), "= 0.25");

        app.on_key(key(KeyCode::F(2)));
        type_text(&mut app, "x");
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.history.last().unwrap().answer.text(), "= 1/4");
    }

    #[test]
    fn enter_on_history_inserts_the_selection() {
        let mut app = app();
        type_text(&mut app, "2+3");
        app.on_key(key(KeyCode::Enter));
        app.on_key(key(KeyCode::Esc));
        app.on_key(key(KeyCode::Up));
        assert_eq!(app.focus, Focus::History);
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.focus, Focus::Input);
        assert!(!input(&app).is_empty());
    }

    #[test]
    fn variables_drawer_inserts_names() {
        let mut app = app();
        type_text(&mut app, "abc := 7");
        app.on_key(key(KeyCode::Enter));
        app.on_key(key(KeyCode::F(3)));
        assert_eq!(app.focus, Focus::Variables);
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.focus, Focus::Input);
        assert_eq!(input(&app), "abc");
    }

    #[test]
    fn hotkey_rebinding_round_trip() {
        let mut app = app();
        app.on_key(key(KeyCode::F(4)));
        assert_eq!(app.tab, Tab::Settings);
        app.on_key(key(KeyCode::F(4)));
        assert_eq!(app.tab, Tab::Calculator);
    }

    #[test]
    fn quit_keys() {
        let mut app = app();
        app.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(app.should_quit);
    }

    #[test]
    fn every_screen_renders_without_panicking() {
        let mut app = app();
        type_text(&mut app, "x := 1/3");
        app.on_key(key(KeyCode::Enter));
        type_text(&mut app, "sqrt(x) + sin(");
        for (w, h) in [(80, 24), (20, 6), (3, 3), (200, 60)] {
            let _ = screen(&mut app, w, h);
        }
        app.on_key(key(KeyCode::F(3)));
        let _ = screen(&mut app, 80, 24);
        app.on_key(key(KeyCode::F(4)));
        let text = screen(&mut app, 80, 24);
        assert!(text.contains("Settings"));
        assert!(text.contains("Typst"));
    }

    #[test]
    fn typst_toggle_changes_rendering_flag() {
        let mut app = app();
        let on = app.settings.typst_rendering;
        app.on_key(key(KeyCode::F(5)));
        assert_ne!(app.settings.typst_rendering, on);
        assert!(!app.persist, "tests never write the real config");
    }
}
