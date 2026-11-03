use std::collections::BTreeMap;
use std::fmt;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Action {
    ToggleRenderMode,
    ToggleVariables,
    NextTab,
    ToggleTypst,
    Quit,
}

impl Action {
    pub const ALL: &[Action] = &[
        Action::ToggleRenderMode,
        Action::ToggleVariables,
        Action::NextTab,
        Action::ToggleTypst,
        Action::Quit,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Action::ToggleRenderMode => "Toggle Decimal / Symbolic",
            Action::ToggleVariables => "Variables drawer (open / focus / close)",
            Action::NextTab => "Switch tab (Calculator / Settings)",
            Action::ToggleTypst => "Toggle Typst rendering",
            Action::Quit => "Quit",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Action::ToggleRenderMode => "toggle_render_mode",
            Action::ToggleVariables => "toggle_variables",
            Action::NextTab => "next_tab",
            Action::ToggleTypst => "toggle_typst",
            Action::Quit => "quit",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|a| a.id() == id)
    }

    pub fn default_binding(self) -> KeyBinding {
        match self {
            Action::ToggleRenderMode => KeyBinding::plain(KeyCode::F(2)),
            Action::ToggleVariables => KeyBinding::plain(KeyCode::F(3)),
            Action::NextTab => KeyBinding::plain(KeyCode::F(4)),
            Action::ToggleTypst => KeyBinding::plain(KeyCode::F(5)),
            Action::Quit => KeyBinding::ctrl('q'),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyBinding {
    pub code: KeyCode,
    pub mods: KeyModifiers,
}

const RESERVED_CTRL: &[char] = &['c', 'h', 'w', 'u', 'a', 'e', 'd', 'i', 'j', 'm', '['];

impl KeyBinding {
    pub fn plain(code: KeyCode) -> Self {
        Self {
            code,
            mods: KeyModifiers::NONE,
        }
    }

    pub fn ctrl(c: char) -> Self {
        Self {
            code: KeyCode::Char(c),
            mods: KeyModifiers::CONTROL,
        }
    }

    pub fn from_event(ev: &KeyEvent) -> Self {
        let mut mods =
            ev.modifiers & (KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SHIFT);
        let code = match ev.code {
            KeyCode::Char(c) => {
                mods.remove(KeyModifiers::SHIFT);
                KeyCode::Char(c.to_ascii_lowercase())
            }
            KeyCode::BackTab => {
                mods.insert(KeyModifiers::SHIFT);
                KeyCode::Tab
            }
            other => other,
        };
        Self { code, mods }
    }

    pub fn validate(&self) -> Result<(), String> {
        if matches!(
            self.code,
            KeyCode::Esc | KeyCode::Enter | KeyCode::Tab | KeyCode::Backspace
        ) {
            return Err("Esc, Enter, Tab and Backspace are used for editing".into());
        }
        let is_f_key = matches!(self.code, KeyCode::F(_));
        let has_modifier = self
            .mods
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
        if !is_f_key && !has_modifier {
            return Err("Use an F-key or a Ctrl/Alt combination".into());
        }
        if let KeyCode::Char(c) = self.code
            && self.mods.contains(KeyModifiers::CONTROL)
            && RESERVED_CTRL.contains(&c)
        {
            return Err(format!("Ctrl+{c} is reserved by the editor"));
        }
        Ok(())
    }

    pub fn parse(s: &str) -> Option<Self> {
        let mut rest = s.trim();
        let mut mods = KeyModifiers::NONE;
        loop {
            let lower = rest.to_ascii_lowercase();
            if let Some(r) = lower.strip_prefix("ctrl+") {
                mods |= KeyModifiers::CONTROL;
                rest = &rest[rest.len() - r.len()..];
            } else if let Some(r) = lower.strip_prefix("alt+") {
                mods |= KeyModifiers::ALT;
                rest = &rest[rest.len() - r.len()..];
            } else if let Some(r) = lower.strip_prefix("shift+") {
                mods |= KeyModifiers::SHIFT;
                rest = &rest[rest.len() - r.len()..];
            } else {
                break;
            }
        }
        let code = match rest.to_ascii_lowercase().as_str() {
            "enter" => KeyCode::Enter,
            "tab" => KeyCode::Tab,
            "esc" => KeyCode::Esc,
            "backspace" => KeyCode::Backspace,
            "delete" => KeyCode::Delete,
            "insert" => KeyCode::Insert,
            "home" => KeyCode::Home,
            "end" => KeyCode::End,
            "pageup" => KeyCode::PageUp,
            "pagedown" => KeyCode::PageDown,
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,
            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            "space" => KeyCode::Char(' '),
            f if f.starts_with('f') && f.len() > 1 => KeyCode::F(f[1..].parse().ok()?),
            _ => {
                let mut it = rest.chars();
                match (it.next(), it.next()) {
                    (Some(c), None) => KeyCode::Char(c.to_ascii_lowercase()),
                    _ => return None,
                }
            }
        };
        Some(Self { code, mods })
    }
}

impl fmt::Display for KeyBinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.mods.contains(KeyModifiers::CONTROL) {
            write!(f, "Ctrl+")?;
        }
        if self.mods.contains(KeyModifiers::ALT) {
            write!(f, "Alt+")?;
        }
        if self.mods.contains(KeyModifiers::SHIFT) {
            write!(f, "Shift+")?;
        }
        match self.code {
            KeyCode::F(n) => write!(f, "F{n}"),
            KeyCode::Char(' ') => write!(f, "Space"),
            KeyCode::Char(c) => write!(f, "{c}"),
            KeyCode::Enter => write!(f, "Enter"),
            KeyCode::Tab => write!(f, "Tab"),
            KeyCode::Esc => write!(f, "Esc"),
            KeyCode::Backspace => write!(f, "Backspace"),
            KeyCode::Delete => write!(f, "Delete"),
            KeyCode::Insert => write!(f, "Insert"),
            KeyCode::Home => write!(f, "Home"),
            KeyCode::End => write!(f, "End"),
            KeyCode::PageUp => write!(f, "PageUp"),
            KeyCode::PageDown => write!(f, "PageDown"),
            KeyCode::Up => write!(f, "Up"),
            KeyCode::Down => write!(f, "Down"),
            KeyCode::Left => write!(f, "Left"),
            KeyCode::Right => write!(f, "Right"),
            other => write!(f, "{other:?}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "BTreeMap<String, String>", into = "BTreeMap<String, String>")]
pub struct Keymap {
    bindings: BTreeMap<Action, KeyBinding>,
}

impl Default for Keymap {
    fn default() -> Self {
        Self {
            bindings: Action::ALL
                .iter()
                .map(|&a| (a, a.default_binding()))
                .collect(),
        }
    }
}

impl Keymap {
    pub fn binding(&self, action: Action) -> KeyBinding {
        self.bindings[&action]
    }

    pub fn action_for(&self, ev: &KeyEvent) -> Option<Action> {
        let pressed = KeyBinding::from_event(ev);
        self.bindings
            .iter()
            .find_map(|(a, b)| (*b == pressed).then_some(*a))
    }

    pub fn rebind(&mut self, action: Action, binding: KeyBinding) -> Result<(), String> {
        binding.validate()?;
        if let Some((other, _)) = self
            .bindings
            .iter()
            .find(|(a, b)| **a != action && **b == binding)
        {
            return Err(format!(
                "{binding} is already used by \"{}\"",
                other.label()
            ));
        }
        self.bindings.insert(action, binding);
        Ok(())
    }

    pub fn reset(&mut self, action: Action) -> Result<(), String> {
        let default = action.default_binding();
        if let Some((other, _)) = self
            .bindings
            .iter()
            .find(|(a, b)| **a != action && **b == default)
        {
            return Err(format!(
                "Default key {default} is used by \"{}\" - rebind that first",
                other.label()
            ));
        }
        self.bindings.insert(action, default);
        Ok(())
    }
}

impl From<BTreeMap<String, String>> for Keymap {
    fn from(saved: BTreeMap<String, String>) -> Self {
        let mut map = Keymap::default();
        for (id, key) in saved {
            if let (Some(action), Some(binding)) = (Action::from_id(&id), KeyBinding::parse(&key)) {
                let _ = map.rebind(action, binding);
            }
        }
        map
    }
}

impl From<Keymap> for BTreeMap<String, String> {
    fn from(map: Keymap) -> Self {
        map.bindings
            .into_iter()
            .map(|(a, b)| (a.id().to_string(), b.to_string()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    #[test]
    fn parse_and_display_roundtrip() {
        for s in [
            "F2",
            "Ctrl+q",
            "Alt+x",
            "Ctrl+Left",
            "Ctrl+Alt+k",
            "Ctrl+Space",
        ] {
            let b = KeyBinding::parse(s).unwrap();
            assert_eq!(b.to_string(), s);
        }
        assert_eq!(KeyBinding::parse("ctrl+Q"), Some(KeyBinding::ctrl('q')));
        assert!(KeyBinding::parse("Ctrl+").is_none());
        assert!(KeyBinding::parse("blah").is_none());
    }

    #[test]
    fn events_are_normalised() {
        let q = KeyBinding::from_event(&ev(
            KeyCode::Char('Q'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        ));
        assert_eq!(q, KeyBinding::ctrl('q'));
    }

    #[test]
    fn defaults_dispatch() {
        let km = Keymap::default();
        assert_eq!(
            km.action_for(&ev(KeyCode::F(2), KeyModifiers::NONE)),
            Some(Action::ToggleRenderMode)
        );
        assert_eq!(
            km.action_for(&ev(KeyCode::Char('q'), KeyModifiers::CONTROL)),
            Some(Action::Quit)
        );
        assert_eq!(
            km.action_for(&ev(KeyCode::Char('q'), KeyModifiers::NONE)),
            None
        );
    }

    #[test]
    fn rebind_validates_and_detects_conflicts() {
        let mut km = Keymap::default();
        assert!(
            km.rebind(Action::Quit, KeyBinding::plain(KeyCode::Char('x')))
                .is_err()
        );
        assert!(
            km.rebind(Action::Quit, KeyBinding::ctrl('w'))
                .unwrap_err()
                .contains("reserved")
        );
        assert!(
            km.rebind(Action::Quit, KeyBinding::plain(KeyCode::F(2)))
                .unwrap_err()
                .contains("already used")
        );
        km.rebind(Action::Quit, KeyBinding::parse("Alt+q").unwrap())
            .unwrap();
        assert_eq!(km.binding(Action::Quit).to_string(), "Alt+q");
        km.reset(Action::Quit).unwrap();
        assert_eq!(km.binding(Action::Quit), KeyBinding::ctrl('q'));
    }

    #[test]
    fn reset_refused_when_default_is_taken() {
        let mut km = Keymap::default();
        km.rebind(
            Action::ToggleRenderMode,
            KeyBinding::parse("Alt+m").unwrap(),
        )
        .unwrap();
        km.rebind(Action::Quit, KeyBinding::plain(KeyCode::F(2)))
            .unwrap();
        assert!(km.reset(Action::ToggleRenderMode).is_err());
    }

    #[test]
    fn serde_is_tolerant() {
        let json = r#"{"quit":"Alt+q","nonsense":"F1","next_tab":"Ctrl+c","toggle_typst":"???"}"#;
        let km: Keymap = serde_json::from_str(json).unwrap();
        assert_eq!(km.binding(Action::Quit).to_string(), "Alt+q");
        assert_eq!(
            km.binding(Action::NextTab),
            Action::NextTab.default_binding()
        );
        assert_eq!(
            km.binding(Action::ToggleTypst),
            Action::ToggleTypst.default_binding()
        );

        let back: Keymap = serde_json::from_str(&serde_json::to_string(&km).unwrap()).unwrap();
        assert_eq!(back, km);
    }
}
