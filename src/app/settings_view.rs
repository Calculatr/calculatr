use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{App, Tab};
use crate::features::editor::{Leave, ListEvent, ListNav, ListSpec};
use crate::features::persistence;
use crate::features::settings::{Action, KeyBinding, Toggle};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsRow {
    Header(&'static str),
    Toggle(Toggle),
    Hotkey(Action),
}

impl SettingsRow {
    pub fn is_selectable(self) -> bool {
        !matches!(self, SettingsRow::Header(_))
    }
}

pub fn settings_rows() -> Vec<SettingsRow> {
    let mut rows = vec![SettingsRow::Header("General")];
    rows.extend(Toggle::ALL.iter().map(|&t| SettingsRow::Toggle(t)));
    rows.push(SettingsRow::Header("Hotkeys"));
    rows.extend(Action::ALL.iter().map(|&a| SettingsRow::Hotkey(a)));
    rows
}

#[derive(Default)]
pub struct SettingsView {
    pub nav: ListNav,
    pub capturing: Option<Action>,
}

impl App {
    pub(super) fn enter_settings_tab(&mut self) {
        self.tab = Tab::Settings;
        self.settings_view.capturing = None;
        let rows = settings_rows();
        let selectable = |i: usize| rows[i].is_selectable();
        let spec = ListSpec {
            len: rows.len(),
            selectable: &selectable,
            allow_filter: false,
        };
        self.settings_view.nav.ensure_cursor(&spec, false);
    }

    pub(super) fn settings_key(&mut self, key: KeyEvent) {
        let rows = settings_rows();
        let selectable = |i: usize| rows[i].is_selectable();
        let spec = ListSpec {
            len: rows.len(),
            selectable: &selectable,
            allow_filter: false,
        };
        let current = self.settings_view.nav.cursor.map(|i| rows[i]);
        let plain = key.modifiers.is_empty();

        match (key.code, current) {
            (KeyCode::Char('r'), Some(SettingsRow::Hotkey(action))) if plain => {
                match self.settings.hotkeys.reset(action) {
                    Ok(()) => {
                        self.save_settings();
                        self.set_status(format!(
                            "{} reset to {}",
                            action.label(),
                            self.settings.hotkeys.binding(action)
                        ));
                    }
                    Err(e) => self.set_error(e),
                }
                return;
            }
            (KeyCode::Char(' '), Some(row)) if plain => {
                self.activate_setting(row);
                return;
            }
            _ => {}
        }

        match self.settings_view.nav.handle_key(key, &spec) {
            ListEvent::Insert(range) => self.activate_setting(rows[*range.start()]),
            ListEvent::Leave(Leave::Left | Leave::Right) => {
                if let Some(SettingsRow::Toggle(t)) = current {
                    self.activate_setting(SettingsRow::Toggle(t));
                }
            }
            ListEvent::Leave(Leave::Escape) => self.tab = Tab::Calculator,
            _ => {}
        }
    }

    fn activate_setting(&mut self, row: SettingsRow) {
        match row {
            SettingsRow::Toggle(t) => {
                self.settings.flip(t);
                self.env.set_complex(self.settings.complex_numbers);
                self.save_settings();
                let state = if self.settings.get(t) { "on" } else { "off" };
                self.set_status(format!("{}: {state}", t.label()));
            }
            SettingsRow::Hotkey(action) => {
                self.settings_view.capturing = Some(action);
                self.set_status(format!(
                    "Press the new key for \"{}\" (Esc cancels)",
                    action.label()
                ));
            }
            SettingsRow::Header(_) => {}
        }
    }

    pub(super) fn capture_hotkey(&mut self, key: KeyEvent) {
        let Some(action) = self.settings_view.capturing else {
            return;
        };
        if key.code == KeyCode::Esc && key.modifiers == KeyModifiers::NONE {
            self.settings_view.capturing = None;
            self.set_status("Rebinding cancelled");
            return;
        }
        let binding = KeyBinding::from_event(&key);
        match self.settings.hotkeys.rebind(action, binding) {
            Ok(()) => {
                self.settings_view.capturing = None;
                self.save_settings();
                self.set_status(format!("\"{}\" is now {binding}", action.label()));
            }
            Err(e) => self.set_error(e),
        }
    }

    pub(super) fn save_settings(&mut self) {
        if !self.persist {
            return;
        }
        if let Err(e) = persistence::save_settings(&self.settings) {
            self.set_error(format!("Could not save settings: {e}"));
        }
    }
}
