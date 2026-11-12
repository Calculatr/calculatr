use std::collections::BTreeSet;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::clipboard::CopyTarget;
use super::{App, Focus, Tab};
use crate::features::core::{self, Evaluation, names};
use crate::features::editor::{EditorConfig, EditorEvent, Leave, ListEvent, ListSpec, Mode};
use crate::features::history::{self, HistoryEntry};
use crate::features::settings::{Action, Setting};
use crate::features::variables::{self, TreeRow};

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
                self.settings.step(Setting::TypstRendering, true);
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
        let is_paste_key = key.modifiers.is_empty() && matches!(key.code, KeyCode::Char('p' | 'P'));
        if is_paste_key && self.editor.mode() != Mode::Insert {
            self.pull_clipboard();
        }
        let event = self.editor.handle_key(key, &cfg);
        self.push_register();
        match event {
            EditorEvent::None => {}
            EditorEvent::Submit => self.submit(),
            EditorEvent::FocusHistory => self.focus_history(),
        }
    }
    // --- clipboard ----------------------------------------------------------

    fn pull_clipboard(&mut self) {
        if !self.settings.system_clipboard {
            return;
        }
        if let Some(text) = self.clipboard.get() {
            let text = single_line(&text);
            self.editor.load_register(text);
        }
    }

    fn push_register(&mut self) {
        if !self.editor.take_register_write() || !self.settings.system_clipboard {
            return;
        }
        let text = self.editor.register().to_string();
        if text.is_empty() {
            return;
        }
        let target = self.clipboard.set(&text);
        if target != CopyTarget::Nowhere {
            self.set_status(format!("Copied to {}", target.label()));
        }
    }

    fn yank(&mut self, text: String) {
        self.editor.set_register(text);
        self.push_register();
    }

    pub fn on_paste(&mut self, text: &str) {
        let text = single_line(text);
        if self.tab != Tab::Calculator {
            return;
        }
        match self.focus {
            Focus::Input => {
                self.editor.paste_and_insert(&text);
                self.status = None;
            }
            Focus::Variables if self.var_filter_active => {
                self.var_filter.push_str(&text);
                self.refresh_var_cursor();
            }
            _ => {}
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
        let input = if text.trim().is_empty() {
            self.history.last().map(|e| e.input.clone())
        } else {
            Some(text)
        };
        if let Some(input) = input {
            if self.defer_eval {
                self.set_status("Calculating …");
                self.pending = Some(input);
            } else {
                self.evaluate_and_record(&input);
            }
        }
        self.editor.reset_after_submit();
        self.history_nav.reset();
        self.history_nav.cursor = None;
    }

    pub(super) fn evaluate_and_record(&mut self, input: &str) {
        let answer = match core::evaluate(&mut self.env, input, self.render_mode) {
            Evaluation::ClearHistory => {
                self.history.clear();
                return;
            }
            Evaluation::Answer(answer) => answer,
            Evaluation::Configure(request) => self.apply_config(request),
        };
        self.history.push(HistoryEntry {
            input: input.trim().to_string(),
            answer,
        });
        self.prune_collapsed_sections();
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
        let rows = self.var_rows();
        let always = |_: usize| true;
        let spec = ListSpec {
            len: rows.len(),
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
                self.yank(text);
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
            .filter_map(|l| {
                history::insert_text_with(&self.history, l, &self.settings.number_format)
            })
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

        let rows = self.var_rows();
        if !self.var_nav.is_visual() && self.tree_key(key, &rows) {
            return;
        }

        let always = |_: usize| true;
        let spec = ListSpec {
            len: rows.len(),
            selectable: &always,
            allow_filter: true,
        };

        match self.var_nav.handle_key(key, &spec) {
            ListEvent::None => {}
            ListEvent::Insert(range) => {
                let text = insertion_text(&rows[range]);
                self.editor.paste_and_insert(&text);
                self.focus_input();
            }
            ListEvent::Yank(range) => {
                let picked = &rows[range];
                let text = picked
                    .iter()
                    .map(TreeRow::insert_text)
                    .collect::<Vec<_>>()
                    .join(", ");
                self.yank(text);
                self.set_status(format!("Yanked {} item(s)", picked.len()));
            }
            ListEvent::Delete(range) => self.delete_var_rows(&rows[range]),
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

    fn tree_key(&mut self, key: KeyEvent, rows: &[TreeRow]) -> bool {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return false;
        }
        let Some(row) = self.var_nav.cursor.and_then(|i| rows.get(i)) else {
            return false;
        };
        let cursor = self.var_nav.cursor.unwrap_or(0);
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') if row.is_section() => {
                self.set_section_expanded(&row.path, !row.is_expanded());
                true
            }
            KeyCode::Right | KeyCode::Char('l') if row.is_section() => {
                if row.is_expanded() {
                    if rows.get(cursor + 1).is_some_and(|r| r.depth > row.depth) {
                        self.var_nav.cursor = Some(cursor + 1);
                    }
                } else {
                    self.set_section_expanded(&row.path, true);
                }
                true
            }
            KeyCode::Left | KeyCode::Char('h') => {
                if row.is_expanded() && self.var_filter.is_empty() {
                    self.set_section_expanded(&row.path, false);
                    return true;
                }
                match names::parent(&row.path).and_then(|p| variables::find(rows, p)) {
                    Some(parent) => {
                        self.var_nav.cursor = Some(parent);
                        true
                    }
                    None => false,
                }
            }
            _ => false,
        }
    }

    fn set_section_expanded(&mut self, path: &str, expanded: bool) {
        if expanded {
            self.var_collapsed.remove(path);
        } else {
            self.var_collapsed.insert(path.to_string());
        }
        self.refresh_var_cursor();
    }

    fn delete_var_rows(&mut self, rows: &[TreeRow]) {
        let mut targets = BTreeSet::new();
        let mut sections = 0;
        for row in rows {
            if row.is_section() {
                sections += 1;
                targets.extend(self.env.section_members(&row.path));
            } else {
                targets.insert(row.path.clone());
            }
        }
        let deleted = targets.iter().filter(|n| self.env.remove_var(n)).count();
        self.prune_collapsed_sections();
        self.refresh_var_cursor();
        let variables = format!("{deleted} variable{}", if deleted == 1 { "" } else { "s" });
        self.set_status(match sections {
            0 => format!("Deleted {variables}"),
            1 => format!("Deleted 1 section ({variables})"),
            n => format!("Deleted {n} sections ({variables})"),
        });
    }

    fn prune_collapsed_sections(&mut self) {
        let env = &self.env;
        self.var_collapsed.retain(|p| env.is_section(p));
    }

    fn refresh_var_cursor(&mut self) {
        let rows = self.var_rows();
        let always = |_: usize| true;
        let spec = ListSpec {
            len: rows.len(),
            selectable: &always,
            allow_filter: true,
        };
        self.var_nav.clamp(&spec);
        self.var_nav.ensure_cursor(&spec, false);
    }
}

fn insertion_text(rows: &[TreeRow]) -> String {
    if let [row] = rows {
        return row.insert_text();
    }
    rows.iter()
        .filter(|r| !r.is_section())
        .map(|r| r.path.clone())
        .collect::<Vec<_>>()
        .join(", ")
}

fn single_line(text: &str) -> String {
    text.split(['\r', '\n'])
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
