use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{App, Tab};
use crate::features::core::{Answer, ConfigRequest};
use crate::features::editor::{Leave, ListEvent, ListNav, ListSpec};
use crate::features::persistence;
use crate::features::settings::{Action, KeyBinding, Setting, SettingGroup};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsRow {
    Header(&'static str),
    Setting(Setting),
    Hotkey(Action),
}

impl SettingsRow {
    pub fn is_selectable(self) -> bool {
        !matches!(self, SettingsRow::Header(_))
    }
}

pub fn settings_rows() -> Vec<SettingsRow> {
    let mut rows = Vec::new();
    for &group in SettingGroup::ALL {
        rows.push(SettingsRow::Header(group.title()));
        rows.extend(Setting::in_group(group).map(SettingsRow::Setting));
    }
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
                self.activate_setting(row, true);
                return;
            }
            _ => {}
        }

        match self.settings_view.nav.handle_key(key, &spec) {
            ListEvent::Insert(range) => self.activate_setting(rows[*range.start()], true),
            ListEvent::Leave(dir @ (Leave::Left | Leave::Right)) => {
                if let Some(row @ SettingsRow::Setting(_)) = current {
                    self.activate_setting(row, dir == Leave::Right);
                }
            }
            ListEvent::Leave(Leave::Escape) => self.tab = Tab::Calculator,
            _ => {}
        }
    }

    fn activate_setting(&mut self, row: SettingsRow, forward: bool) {
        match row {
            SettingsRow::Setting(setting) => {
                self.settings.step(setting, forward);
                self.settings_changed();
                let message = self.setting_message(setting);
                self.set_status(message);
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

    pub(super) fn apply_config(&mut self, request: ConfigRequest) -> Answer {
        let (key, value) = match request {
            ConfigRequest::Show => return Answer::Message(self.settings.overview()),
            ConfigRequest::Cycle(key) => (key, None),
            ConfigRequest::Set(key, value) => (key, Some(value)),
        };
        let Some(setting) = Setting::from_key(&key) else {
            let known: Vec<&str> = Setting::ALL.iter().map(|s| s.key()).collect();
            return Answer::Error(format!(
                "Unknown setting '{key}'; available: {}",
                known.join(", ")
            ));
        };
        let result = match value {
            None => {
                self.settings.step(setting, true);
                Ok(())
            }
            Some(value) => self.settings.set_value(setting, &value),
        };
        match result {
            Ok(()) => {
                self.settings_changed();
                Answer::Message(self.setting_message(setting))
            }
            Err(e) => Answer::Error(e),
        }
    }

    fn setting_message(&self, setting: Setting) -> String {
        let mut message = format!(
            "{}: {}",
            setting.label(),
            self.settings.value_label(setting)
        );
        if let Some(reason) = self.settings.inactive_reason(setting) {
            message.push_str(&format!(" ({reason})"));
        }
        message
    }

    pub(super) fn settings_changed(&mut self) {
        self.env.set_complex(self.settings.complex_numbers);
        self.save_settings();
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
