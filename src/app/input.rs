use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{App, Focus, Tab};
use crate::features::core::{self, Evaluation};
use crate::features::editor::{EditorConfig, EditorEvent, Leave, ListEvent, ListSpec};
use crate::features::history::{self, HistoryEntry};
use crate::features::settings::{Action, Toggle};

impl App {
    pub fn on_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.should_quit = true;
            return;
        }

        if self.tab == Tab::Settings && self.settings_view.capturing.is_some() {
            self.capture_hotkey(key);
            return;
        }

        if let Some(action) = self.settings.hotkeys.action_for(&key) {
            self.run_action(action);
            return;
        }

        self.status = None;
        match self.tab {
            Tab::Settings => self.settings_key(key),
            Tab::Calculator => match self.focus {
                Focus::Input => self.input_key(key),
                Focus::History => self.history_key(key),
                Focus::Variables => self.variables_key(key),
            },
        }
    }

    // --- global actions -----------------------------------------------------

    fn run_action(&mut self, action: Action) {
        match action {
            Action::Quit => self.should_quit = true,
            Action::ToggleRenderMode => {
                self.render_mode = self.render_mode.toggled();
                self.set_status(format!("Mode: {:?}", self.render_mode));
            }
            Action::ToggleTypst => {
                self.settings.flip(Toggle::TypstRendering);
                self.save_settings();
                let state = if self.settings.typst_rendering {
                    "on"
                } else {
                    "off"
                };
                self.set_status(format!("Typst rendering: {state}"));
            }
            Action::NextTab => match self.tab {
                Tab::Calculator => self.enter_settings_tab(),
                Tab::Settings => self.tab = Tab::Calculator,
            },
            Action::ToggleVariables => {
                self.tab = Tab::Calculator;
                if !self.variables_open {
                    self.variables_open = true;
                    self.focus_variables();
                } else if self.focus != Focus::Variables {
                    self.focus_variables();
                } else {
                    self.variables_open = false;
                    self.var_filter_active = false;
                    self.focus = Focus::Input;
                }
            }
        }
    }

    // --- input pane -----------------------------------------------------------

    fn input_key(&mut self, key: KeyEvent) {
        let candidates = core::completions(&self.env);
        let cfg = EditorConfig {
            auto_close: self.settings.auto_close_brackets,
            candidates: &candidates,
        };
        match self.editor.handle_key(key, &cfg) {
            EditorEvent::None => {}
            EditorEvent::Submit => self.submit(),
            EditorEvent::FocusHistory => self.focus_history(),
        }
    }

    fn submit(&mut self) {
        if self.editor.buffer.has_placeholder() {
            self.editor.buffer.select_next_placeholder();
            self.editor.enter_insert_mode();
            self.set_status("Fill in the remaining <> placeholders (Tab jumps to the next)");
            return;
        }

        let text = self.editor.buffer.text();
        if text.trim().is_empty() {
            if let Some(last) = self.history.last().map(|e| e.input.clone()) {
                self.evaluate_and_record(&last);
            }
        } else {
            self.evaluate_and_record(&text);
        }
        self.editor.reset_after_submit();
        self.history_nav.reset();
        self.history_nav.cursor = None;
    }

    fn evaluate_and_record(&mut self, input: &str) {
        match core::evaluate(&mut self.env, input, self.render_mode) {
            Evaluation::ClearHistory => self.history.clear(),
            Evaluation::Answer(answer) => self.history.push(HistoryEntry {
                input: input.trim().to_string(),
                answer,
            }),
        }
    }

    // --- focus changes ------------------------------------------------------

    fn focus_input(&mut self) {
        self.focus = Focus::Input;
        self.history_nav.reset();
        self.history_nav.cursor = None;
    }

    pub(super) fn focus_history(&mut self) {
        let selectable = |i: usize| history::is_selectable(&self.history, i);
        let spec = ListSpec {
            len: history::line_count(&self.history),
            selectable: &selectable,
            allow_filter: false,
        };
        if spec.last().is_none() {
            return;
        }
        self.history_nav.ensure_cursor(&spec, true);
        self.editor.leave();
        self.focus = Focus::History;
    }

    fn focus_variables(&mut self) {
        let names = self.env.sorted_names(&self.var_filter);
        let always = |_: usize| true;
        let spec = ListSpec {
            len: names.len(),
            selectable: &always,
            allow_filter: true,
        };
        self.var_nav.ensure_cursor(&spec, false);
        self.editor.leave();
        self.focus = Focus::Variables;
    }

    // --- history pane -------------------------------------------------------

    fn history_key(&mut self, key: KeyEvent) {
        let len = history::line_count(&self.history);
        let event = {
            let selectable = |i: usize| history::is_selectable(&self.history, i);
            let spec = ListSpec {
                len,
                selectable: &selectable,
                allow_filter: false,
            };
            self.history_nav.handle_key(key, &spec)
        };

        match event {
            ListEvent::None => {}
            ListEvent::Insert(range) => {
                let text = self.joined_history_text(range);
                self.editor.paste_and_insert(&text);
                self.focus_input();
            }
            ListEvent::Yank(range) => {
                let text = self.joined_history_text(range.clone());
                self.editor.set_register(text);
                self.set_status(format!("Yanked {} line(s)", range.count()));
            }
            ListEvent::Delete(range) => {
                let before = history::line_count(&self.history);
                history::remove_lines(&mut self.history, range);
                let removed = (before - history::line_count(&self.history)) / 2;
                let selectable = |i: usize| history::is_selectable(&self.history, i);
                let spec = ListSpec {
                    len: history::line_count(&self.history),
                    selectable: &selectable,
                    allow_filter: false,
                };
                self.history_nav.clamp(&spec);
                self.set_status(format!(
                    "Deleted {removed} entr{}",
                    if removed == 1 { "y" } else { "ies" }
                ));
                if self.history_nav.cursor.is_none() {
                    self.focus_input();
                }
            }
            ListEvent::Leave(Leave::Down | Leave::Escape) => self.focus_input(),
            ListEvent::Leave(Leave::Right) => {
                if self.variables_open {
                    self.focus_variables();
                }
            }
            ListEvent::Leave(Leave::Left) | ListEvent::StartFilter => {}
        }
    }

    fn joined_history_text(&self, lines: std::ops::RangeInclusive<usize>) -> String {
        lines
            .filter_map(|l| history::insert_text(&self.history, l))
            .collect::<Vec<_>>()
            .join(", ")
    }

    // --- variables drawer ---------------------------------------------------

    fn variables_key(&mut self, key: KeyEvent) {
        if self.var_filter_active {
            match key.code {
                KeyCode::Esc | KeyCode::Enter => self.var_filter_active = false,
                KeyCode::Backspace => {
                    self.var_filter.pop();
                }
                KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.var_filter.push(c);
                }
                _ => {}
            }
            self.refresh_var_cursor();
            return;
        }

        let names = self.env.sorted_names(&self.var_filter);
        let always = |_: usize| true;
        let spec = ListSpec {
            len: names.len(),
            selectable: &always,
            allow_filter: true,
        };

        match self.var_nav.handle_key(key, &spec) {
            ListEvent::None => {}
            ListEvent::Insert(range) => {
                let text = names[range].join(", ");
                self.editor.paste_and_insert(&text);
                self.focus_input();
            }
            ListEvent::Yank(range) => {
                self.editor.set_register(names[range.clone()].join(", "));
                self.set_status(format!("Yanked {} variable(s)", range.count()));
            }
            ListEvent::Delete(range) => {
                let count = range.clone().count();
                for name in &names[range] {
                    self.env.remove_var(name);
                }
                self.refresh_var_cursor();
                self.set_status(format!("Deleted {count} variable(s)"));
            }
            ListEvent::StartFilter => {
                self.var_filter_active = true;
                self.var_filter.clear();
                self.var_nav.cursor = None;
            }
            ListEvent::Leave(Leave::Left) => {
                self.focus_history();
                if self.focus != Focus::History {
                    self.focus_input();
                }
            }
            ListEvent::Leave(Leave::Escape) => self.focus_input(),
            ListEvent::Leave(Leave::Right | Leave::Down) => {}
        }
    }

    fn refresh_var_cursor(&mut self) {
        let names = self.env.sorted_names(&self.var_filter);
        let always = |_: usize| true;
        let spec = ListSpec {
            len: names.len(),
            selectable: &always,
            allow_filter: true,
        };
        self.var_nav.clamp(&spec);
        self.var_nav.ensure_cursor(&spec, false);
    }
}
