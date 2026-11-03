pub mod keymap;

use serde::{Deserialize, Serialize};

pub use keymap::{Action, KeyBinding, Keymap};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub typst_rendering: bool,
    pub auto_close_brackets: bool,
    pub system_clipboard: bool,
    pub hotkeys: Keymap,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            typst_rendering: true,
            auto_close_brackets: true,
            system_clipboard: true,
            hotkeys: Keymap::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toggle {
    TypstRendering,
    AutoCloseBrackets,
    SystemClipboard,
}

impl Toggle {
    pub const ALL: &[Toggle] = &[
        Toggle::TypstRendering,
        Toggle::AutoCloseBrackets,
        Toggle::SystemClipboard,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Toggle::TypstRendering => "Typst rendering",
            Toggle::AutoCloseBrackets => "Auto-close brackets",
            Toggle::SystemClipboard => "System clipboard",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Toggle::TypstRendering => {
                "Typeset expressions and results as math (needs an image-capable terminal)"
            }
            Toggle::AutoCloseBrackets => {
                "Typing ( inserts ) at a sensible place; typing ) steps over an existing one"
            }
            Toggle::SystemClipboard => {
                "Yanks also go to the system clipboard and p reads from it (terminal paste works in insert mode)"
            }
        }
    }
}

impl Settings {
    pub fn get(&self, toggle: Toggle) -> bool {
        match toggle {
            Toggle::TypstRendering => self.typst_rendering,
            Toggle::AutoCloseBrackets => self.auto_close_brackets,
            Toggle::SystemClipboard => self.system_clipboard,
        }
    }

    pub fn flip(&mut self, toggle: Toggle) {
        match toggle {
            Toggle::TypstRendering => self.typst_rendering = !self.typst_rendering,
            Toggle::AutoCloseBrackets => self.auto_close_brackets = !self.auto_close_brackets,
            Toggle::SystemClipboard => self.system_clipboard = !self.system_clipboard,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_use_defaults() {
        let s: Settings = serde_json::from_str(r#"{"typst_rendering": false}"#).unwrap();
        assert!(!s.typst_rendering);
        assert!(s.auto_close_brackets);
        assert_eq!(s.hotkeys, Keymap::default());
    }

    #[test]
    fn toggles_flip() {
        let mut s = Settings::default();
        s.flip(Toggle::TypstRendering);
        assert!(!s.get(Toggle::TypstRendering));
    }
}
