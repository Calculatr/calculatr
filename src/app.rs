use crossterm::{
    ExecutableCommand,
    event::{self, Event, KeyCode},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol};
use std::collections::HashMap;
use std::io::{self, stdout};

use crate::features::core::{self, Environment, RenderMode};
use crate::features::persistence;
use crate::features::tui;
use crate::features::tui::completion::{self, CompState};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ActivePane {
    HistoryInput,
    VariablesDrawer,
}

pub struct TypstImage {
    pub proto: StatefulProtocol,
    pub px_w: u32,
    pub px_h: u32,
}

pub struct App {
    pub input: String,
    pub history: Vec<String>,
    pub should_quit: bool,
    pub env: Environment,
    pub render_mode: RenderMode,
    pub selected: Option<usize>,
    pub comp: Option<CompState>,
    pub variables_drawer_open: bool,
    pub active_pane: ActivePane,
    pub var_filter: String,
    pub var_filter_active: bool,
    pub var_selected: Option<usize>,
    pub picker: Picker,
    pub images: HashMap<String, Option<TypstImage>>,
}

impl App {
    pub fn new(picker: Picker) -> Self {
        if let Some(state) = persistence::load_state() {
            Self {
                input: String::new(),
                history: state.history,
                should_quit: false,
                env: state.env,
                render_mode: state.render_mode,
                selected: None,
                comp: None,
                variables_drawer_open: false,
                active_pane: ActivePane::HistoryInput,
                var_filter: String::new(),
                var_filter_active: false,
                var_selected: None,
                picker,
                images: HashMap::new(),
            }
        } else {
            Self {
                input: String::new(),
                history: Vec::new(),
                should_quit: false,
                env: Environment::new(),
                render_mode: RenderMode::Decimal,
                selected: None,
                comp: None,
                variables_drawer_open: false,
                active_pane: ActivePane::HistoryInput,
                var_filter: String::new(),
                var_filter_active: false,
                var_selected: None,
                picker,
                images: HashMap::new(),
            }
        }
    }
}

fn is_selectable(line: &str) -> bool {
    !line.trim().is_empty() && !line.starts_with("-- Mode")
}

fn insertable(line: &str) -> String {
    if let Some(cmd) = line.strip_prefix("> ") {
        cmd.trim().to_string()
    } else {
        let t = line.trim();
        t.strip_prefix('=').unwrap_or(t).trim().to_string()
    }
}

fn handle_tab(app: &mut App) {
    let cands = core::completions(&app.env);
    let (start, word) = completion::current_word(&app.input);

    let (prefix, cycling) = match &app.comp {
        Some(c) => (c.prefix.clone(), true),
        None => (word.to_string(), false),
    };

    let matches = completion::matching(&cands, &prefix);
    if matches.is_empty() {
        app.comp = None;
        return;
    }

    if !cycling {
        if matches.len() == 1 {
            let m = matches[0];
            let mut text = m.name.clone();
            if m.is_function {
                text.push('(');
            }
            app.input.truncate(start);
            app.input.push_str(&text);
            return;
        }
        let lcp = completion::common_prefix(&matches);
        if lcp.len() > prefix.len() {
            app.input.truncate(start);
            app.input.push_str(&lcp);
            return;
        }
    }

    let idx = match &app.comp {
        Some(c) => (c.idx + 1) % matches.len(),
        None => 0,
    };
    let name = matches[idx].name.clone();
    app.input.truncate(start);
    app.input.push_str(&name);
    app.comp = Some(CompState { prefix, idx });
}

pub fn run() -> io::Result<()> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    let picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());
    let mut app = App::new(picker);

    while !app.should_quit {
        terminal.draw(|f| tui::draw(f, &mut app))?;

        if event::poll(std::time::Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
        {
            if key.code != KeyCode::Tab && app.active_pane == ActivePane::HistoryInput {
                app.comp = None;
            }

            match key.code {
                KeyCode::F(3) => {
                    app.variables_drawer_open = !app.variables_drawer_open;
                    if app.variables_drawer_open {
                        app.active_pane = ActivePane::VariablesDrawer;
                    } else {
                        app.active_pane = ActivePane::HistoryInput;
                        app.var_filter_active = false;
                    }
                }
                KeyCode::Tab => {
                    if app.variables_drawer_open && app.active_pane == ActivePane::HistoryInput {
                        let cands = core::completions(&app.env);
                        let (_, word) = completion::current_word(&app.input);
                        let prefix = match &app.comp {
                            Some(c) => c.prefix.as_str(),
                            None => word,
                        };
                        let matches = completion::matching(&cands, prefix);

                        if app.comp.is_some() || !matches.is_empty() {
                            handle_tab(&mut app);
                        } else {
                            app.active_pane = ActivePane::VariablesDrawer;
                            app.var_filter_active = false;
                        }
                    } else if app.variables_drawer_open
                        && app.active_pane == ActivePane::VariablesDrawer
                    {
                        app.active_pane = ActivePane::HistoryInput;
                        app.var_filter_active = false;
                    } else if app.selected.is_none() {
                        handle_tab(&mut app);
                    }
                }
                _ => {
                    if app.variables_drawer_open && app.active_pane == ActivePane::VariablesDrawer {
                        if app.var_filter_active {
                            match key.code {
                                KeyCode::Esc | KeyCode::Enter => {
                                    app.var_filter_active = false;
                                }
                                KeyCode::Backspace => {
                                    app.var_filter.pop();
                                }
                                KeyCode::Char(c) => {
                                    app.var_filter.push(c);
                                }
                                _ => {}
                            }
                        } else {
                            let mut vars: Vec<_> = app.env.get_vars().iter().collect();
                            vars.sort_by(|a, b| a.0.cmp(b.0));
                            let filtered: Vec<_> = vars
                                .into_iter()
                                .filter(|(name, _)| name.contains(&app.var_filter))
                                .collect();

                            match key.code {
                                KeyCode::Esc => {
                                    app.variables_drawer_open = false;
                                    app.active_pane = ActivePane::HistoryInput;
                                }
                                KeyCode::Char('/') => {
                                    app.var_filter_active = true;
                                    app.var_filter.clear();
                                    app.var_selected = None;
                                }
                                KeyCode::Up => {
                                    if !filtered.is_empty() {
                                        app.var_selected = match app.var_selected {
                                            None => Some(filtered.len() - 1),
                                            Some(0) => Some(0),
                                            Some(s) => Some(s - 1),
                                        };
                                    }
                                }
                                KeyCode::Down => {
                                    if !filtered.is_empty() {
                                        app.var_selected = match app.var_selected {
                                            None => Some(0),
                                            Some(s) => {
                                                if s + 1 < filtered.len() {
                                                    Some(s + 1)
                                                } else {
                                                    Some(s)
                                                }
                                            }
                                        };
                                    }
                                }
                                KeyCode::Enter => {
                                    if let Some(s) = app.var_selected
                                        && let Some((name, _)) = filtered.get(s)
                                    {
                                        app.input.push_str(name);
                                        app.active_pane = ActivePane::HistoryInput;
                                    }
                                }
                                KeyCode::Delete => {
                                    let filtered_names: Vec<String> = {
                                        let mut vars: Vec<_> = app.env.get_vars().iter().collect();
                                        vars.sort_by(|a, b| a.0.cmp(b.0));
                                        vars.into_iter()
                                            .filter(|(name, _)| name.contains(&app.var_filter))
                                            .map(|(name, _)| name.clone())
                                            .collect()
                                    };

                                    if let Some(s) = app.var_selected
                                        && let Some(name_to_remove) = filtered_names.get(s)
                                    {
                                        app.env.remove_var(name_to_remove);
                                        if filtered_names.len() <= 1 {
                                            app.var_selected = None;
                                        } else if s > 0 && s >= filtered_names.len() - 1 {
                                            app.var_selected = Some(s - 1);
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    } else {
                        match key.code {
                            KeyCode::Esc => {
                                if app.selected.is_some() {
                                    app.selected = None;
                                } else {
                                    app.should_quit = true;
                                }
                            }
                            KeyCode::F(2) => {
                                app.render_mode = match app.render_mode {
                                    RenderMode::Symbolic => RenderMode::Decimal,
                                    RenderMode::Decimal => RenderMode::Symbolic,
                                };
                                // app.history
                                //     .push(format!("-- Mode: {:?} --", app.render_mode));
                                app.selected = None;
                            }
                            KeyCode::Char(c) => {
                                app.selected = None;
                                app.input.push(c);
                            }
                            KeyCode::Backspace => {
                                app.selected = None;
                                app.input.pop();
                            }
                            KeyCode::Up => {
                                app.selected = match app.selected {
                                    None => app.history.iter().rposition(|l| is_selectable(l)),
                                    Some(s) => (0..s)
                                        .rev()
                                        .find(|&i| is_selectable(&app.history[i]))
                                        .or(Some(s)),
                                };
                            }
                            KeyCode::Down => {
                                if let Some(s) = app.selected {
                                    app.selected = (s + 1..app.history.len())
                                        .find(|&i| is_selectable(&app.history[i]));
                                }
                            }
                            KeyCode::Enter => {
                                if let Some(s) = app.selected {
                                    let text = insertable(&app.history[s]);
                                    app.input.push_str(&text);
                                    app.selected = None;
                                } else if app.input.is_empty() {
                                    if let Some(last_cmd) =
                                        app.history.iter().rev().find(|line| line.starts_with("> "))
                                    {
                                        let query =
                                            last_cmd.strip_prefix("> ").unwrap_or("").to_string();
                                        app.history.push(format!("> {}", query));
                                        let result =
                                            core::evaluate(&mut app.env, &query, app.render_mode);
                                        app.history.push(format!("  {}", result));
                                    }
                                } else {
                                    let query = app.input.clone();
                                    app.history.push(format!("> {}", query));

                                    let result =
                                        core::evaluate(&mut app.env, &query, app.render_mode);
                                    app.history.push(format!("  {}", result));

                                    app.input.clear();
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    persistence::save_state(app.render_mode, &app.history, &app.env);

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    Ok(())
}
