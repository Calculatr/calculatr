mod clipboard;
mod input;
mod settings_view;

use std::collections::HashSet;
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
use crate::features::tui::{self, display::DisplayCache, images::ImageCache, theme::Theme};
use crate::features::variables::{self, TreeRow};

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
    pub var_collapsed: HashSet<String>,
    pub settings: Settings,
    pub settings_view: SettingsView,
    pub status: Option<Status>,
    pub theme: Theme,
    pub picker: Picker,
    pub images: ImageCache,
    pub display: DisplayCache,
    pub should_quit: bool,
    pub(crate) clipboard: Clipboard,
    persist: bool,
    cursor_shape: Option<SetCursorStyle>,
    pending: Option<String>,
    defer_eval: bool,
}

impl App {
    pub fn new(picker: Picker) -> Self {
        let mut app = Self::build(
            picker,
            persistence::load_settings(),
            persistence::load_state(),
        );
        app.persist = true;
        app.defer_eval = true;
        app.display.enable_background();
        app.clipboard = Clipboard::system();
        app
    }

    pub fn build(picker: Picker, settings: Settings, state: Option<SavedState>) -> Self {
        let (render_mode, history, mut env) = match state {
            Some(s) => (s.render_mode, s.history, s.env),
            None => (RenderMode::Decimal, Vec::new(), Environment::new()),
        };
        env.set_complex(settings.complex_numbers);
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
            var_collapsed: HashSet::new(),
            settings,
            settings_view: SettingsView::default(),
            status: None,
            theme: Theme::cyberpunk(),
            picker,
            images: ImageCache::new(),
            display: DisplayCache::new(),
            should_quit: false,
            clipboard: Clipboard::disabled(),
            persist: false,
            cursor_shape: None,
            pending: None,
            defer_eval: false,
        }
    }

    pub fn var_rows(&self) -> Vec<TreeRow> {
        variables::build(
            &self.env.sorted_names(),
            &self.var_collapsed,
            &self.var_filter,
        )
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

        if let Some(input) = app.pending.take() {
            app.status = None;
            app.evaluate_and_record(&input);
            dirty = true;
            continue;
        }

        let timeout = if app.display.is_busy() {
            Duration::from_millis(15)
        } else {
            Duration::from_millis(250)
        };
        if event::poll(timeout)? {
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
        dirty |= app.display.poll();
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
        assert_eq!(app.history.last().unwrap().answer.text(), "0.25");
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
        assert_eq!(app.history.last().unwrap().answer.text(), "0.25");

        app.on_key(key(KeyCode::F(2)));
        type_text(&mut app, "x");
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.history.last().unwrap().answer.text(), "1/4");
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

    fn submit(app: &mut App, text: &str) {
        type_text(app, text);
        app.on_key(key(KeyCode::Enter));
    }

    fn last_answer(app: &App) -> String {
        app.history
            .last()
            .unwrap()
            .answer
            .text_with(&app.settings.number_format)
    }

    #[test]
    fn settings_commands_change_the_number_format() {
        let mut app = app();
        submit(&mut app, "x := 12345.678");
        submit(&mut app, "notation(si)");
        assert!(last_answer(&app).starts_with("Notation: SI prefix"));
        submit(&mut app, "decimals(1)");
        submit(&mut app, "x");
        assert_eq!(last_answer(&app), "12.3k");
        let result = app.history.len() - 1;
        let shown = |app: &App| {
            app.history[result]
                .answer
                .text_with(&app.settings.number_format)
        };
        submit(&mut app, "set(notation, sci)");
        assert_eq!(shown(&app), "1.2*10^4");
        submit(&mut app, "set(notation, e)");
        assert_eq!(shown(&app), "1.2e4");
        submit(&mut app, "exponent(eng)");
        assert_eq!(shown(&app), "12.3e3");
        submit(&mut app, "set(precision, auto)");
        assert_eq!(shown(&app), "12.345678e3");
        submit(&mut app, "set(nonsense, 1)");
        assert!(last_answer(&app).contains("Unknown setting"));
        submit(&mut app, "decimals(99)");
        assert!(last_answer(&app).contains("Invalid value"));
        submit(&mut app, "settings()");
        assert!(last_answer(&app).contains("notation=e"));
    }

    #[test]
    fn inserted_results_stay_parseable() {
        let mut app = app();
        submit(&mut app, "notation(si)");
        submit(&mut app, "1500");
        assert_eq!(last_answer(&app), "1.5k");
        app.on_key(key(KeyCode::Esc));
        app.on_key(key(KeyCode::Up));
        app.on_key(key(KeyCode::Enter));
        assert_eq!(input(&app), "1.5*10^3");
    }

    #[test]
    fn settings_tab_steps_choices_with_arrows() {
        let mut app = app();
        app.on_key(key(KeyCode::F(4)));
        let rows = settings_rows();
        let notation = rows
            .iter()
            .position(|r| *r == SettingsRow::Setting(crate::features::settings::Setting::Notation))
            .unwrap();
        app.settings_view.nav.cursor = Some(notation);
        app.on_key(key(KeyCode::Right));
        assert_eq!(
            app.settings.number_format.notation,
            crate::features::core::Notation::Power
        );
        app.on_key(key(KeyCode::Left));
        app.on_key(key(KeyCode::Left));
        assert_eq!(
            app.settings.number_format.notation,
            crate::features::core::Notation::SiPrefix
        );
        let text = screen(&mut app, 100, 30);
        assert!(text.contains("Number format"));
        assert!(text.contains("SI prefix"));
    }

    #[test]
    fn variables_drawer_shows_and_folds_sections() {
        let mut app = app();
        submit(&mut app, "physics.g := 9.81");
        submit(&mut app, "physics.a1.v := 2");
        submit(&mut app, "x := 1");
        app.on_key(key(KeyCode::F(3)));
        let text = screen(&mut app, 100, 20);
        assert!(text.contains("▾ physics (2)"), "{text}");
        assert!(text.contains("▾ a1 (1)"));

        assert_eq!(app.var_nav.cursor, Some(0));
        app.on_key(key(KeyCode::Enter));
        assert!(screen(&mut app, 100, 20).contains("▸ physics (2)"));
        assert_eq!(app.var_rows().len(), 2);
        app.on_key(key(KeyCode::Char('l')));
        app.on_key(key(KeyCode::Char('l')));
        assert_eq!(app.var_nav.cursor, Some(1));
        app.on_key(key(KeyCode::Char('h')));
        assert!(app.var_collapsed.contains("physics.a1"));
        app.on_key(key(KeyCode::Char('h')));
        assert_eq!(app.var_nav.cursor, Some(0));
        app.on_key(key(KeyCode::Char('l')));
        app.on_key(key(KeyCode::Char('j')));
        app.on_key(key(KeyCode::Enter));
        assert_eq!(input(&app), "physics.g");
    }

    #[test]
    fn deleting_a_section_in_the_drawer_is_recursive() {
        let mut app = app();
        submit(&mut app, "physics.g := 9.81");
        submit(&mut app, "physics.a1.v := 2");
        submit(&mut app, "x := 1");
        app.on_key(key(KeyCode::F(3)));
        app.on_key(key(KeyCode::Char('d')));
        app.on_key(key(KeyCode::Char('d')));
        assert_eq!(app.env.sorted_names(), vec!["x".to_string()]);
        assert_eq!(
            app.status.as_ref().unwrap().text,
            "Deleted 1 section (2 variables)"
        );
    }

    #[test]
    fn units_follow_the_number_format() {
        let mut app = app();
        submit(&mut app, "1500_m");
        assert_eq!(last_answer(&app), "1500_m");
        let result = app.history.len() - 1;
        submit(&mut app, "notation(si)");
        let shown = |app: &App| {
            app.history[result]
                .answer
                .text_with(&app.settings.number_format)
        };
        assert_eq!(shown(&app), "1.5_km");
        submit(&mut app, "notation(sci)");
        assert_eq!(shown(&app), "(1.5*10^3)_m");
        submit(&mut app, "notation(si)");
        submit(&mut app, "0.0047_A");
        assert_eq!(last_answer(&app), "4.7_mA");
        submit(&mut app, "2_kN*3_m");
        assert_eq!(last_answer(&app), "6_kJ");
    }

    #[test]
    fn inserted_quantities_parse_back() {
        let mut app = app();
        submit(&mut app, "notation(sci)");
        submit(&mut app, "1500_m");
        app.on_key(key(KeyCode::Esc));
        app.on_key(key(KeyCode::Up));
        app.on_key(key(KeyCode::Enter));
        assert_eq!(input(&app), "(1.5*10^3)_m");
        app.on_key(key(KeyCode::Enter));
        assert_eq!(last_answer(&app), "(1.5*10^3)_m");
        submit(&mut app, "notation(si)");
        app.on_key(key(KeyCode::Esc));
        app.on_key(key(KeyCode::Up));
        app.on_key(key(KeyCode::Char('k')));
        app.on_key(key(KeyCode::Enter));
        assert_eq!(input(&app), "1.5_km");
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
