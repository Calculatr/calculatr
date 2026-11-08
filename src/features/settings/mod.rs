pub mod keymap;

use serde::{Deserialize, Serialize};

use crate::features::core::notation::MAX_DECIMAL_PLACES;
use crate::features::core::{ExponentStep, Notation, NumberFormat};

pub use keymap::{Action, KeyBinding, Keymap};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub typst_rendering: bool,
    pub auto_close_brackets: bool,
    pub system_clipboard: bool,
    pub complex_numbers: bool,
    pub number_format: NumberFormat,
    pub hotkeys: Keymap,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            typst_rendering: true,
            auto_close_brackets: true,
            system_clipboard: true,
            complex_numbers: false,
            number_format: NumberFormat::default(),
            hotkeys: Keymap::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Setting registry
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingGroup {
    General,
    NumberFormat,
}

impl SettingGroup {
    pub const ALL: &[SettingGroup] = &[SettingGroup::General, SettingGroup::NumberFormat];

    pub fn title(self) -> &'static str {
        match self {
            SettingGroup::General => "General",
            SettingGroup::NumberFormat => "Number format",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setting {
    TypstRendering,
    AutoCloseBrackets,
    SystemClipboard,
    ComplexNumbers,
    DecimalPlaces,
    Notation,
    ExponentStep,
    ExtendedPrefixes,
}

impl Setting {
    pub const ALL: &[Setting] = &[
        Setting::TypstRendering,
        Setting::AutoCloseBrackets,
        Setting::SystemClipboard,
        Setting::ComplexNumbers,
        Setting::DecimalPlaces,
        Setting::Notation,
        Setting::ExponentStep,
        Setting::ExtendedPrefixes,
    ];

    pub fn group(self) -> SettingGroup {
        match self {
            Setting::TypstRendering
            | Setting::AutoCloseBrackets
            | Setting::SystemClipboard
            | Setting::ComplexNumbers => SettingGroup::General,
            Setting::DecimalPlaces
            | Setting::Notation
            | Setting::ExponentStep
            | Setting::ExtendedPrefixes => SettingGroup::NumberFormat,
        }
    }

    pub fn in_group(group: SettingGroup) -> impl Iterator<Item = Setting> {
        Self::ALL
            .iter()
            .copied()
            .filter(move |s| s.group() == group)
    }

    pub fn label(self) -> &'static str {
        match self {
            Setting::TypstRendering => "Typst rendering",
            Setting::AutoCloseBrackets => "Auto-close brackets",
            Setting::SystemClipboard => "System clipboard",
            Setting::ComplexNumbers => "Complex numbers",
            Setting::DecimalPlaces => "Decimal places",
            Setting::Notation => "Notation",
            Setting::ExponentStep => "Exponent step",
            Setting::ExtendedPrefixes => "Extended SI prefixes",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Setting::TypstRendering => {
                "Typeset expressions and results as math (needs an image-capable terminal)"
            }
            Setting::AutoCloseBrackets => {
                "Typing ( inserts ) at a sensible place; typing ) steps over an existing one"
            }
            Setting::SystemClipboard => {
                "Yanks also go to the system clipboard and p reads from it (terminal paste works in insert mode)"
            }
            Setting::ComplexNumbers => {
                "i is the imaginary unit and sqrt(-4), ln(-1) ... give complex results (off: i is a normal variable)"
            }
            Setting::DecimalPlaces => {
                "Round decimal results (the mantissa when an exponent is shown); auto keeps every exact digit"
            }
            Setting::Notation => {
                "How large and small decimal results are written: 1500, 1.5*10^3, 1.5e3 or 1.5k"
            }
            Setting::ExponentStep => {
                "Scientific: one digit before the point (15000 = 1.5e4). Engineering: exponents in steps of 3 (15e3)"
            }
            Setting::ExtendedPrefixes => {
                "Also use c (centi), d (deci), da (deca) and h (hecto) with SI prefixes"
            }
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Setting::TypstRendering => "typst",
            Setting::AutoCloseBrackets => "autoclose",
            Setting::SystemClipboard => "clipboard",
            Setting::ComplexNumbers => "complex",
            Setting::DecimalPlaces => "decimals",
            Setting::Notation => "notation",
            Setting::ExponentStep => "exponent",
            Setting::ExtendedPrefixes => "extprefixes",
        }
    }

    fn aliases(self) -> &'static [&'static str] {
        match self {
            Setting::TypstRendering => &["typst_rendering", "rendering"],
            Setting::AutoCloseBrackets => &["auto_close", "brackets"],
            Setting::SystemClipboard => &["system_clipboard"],
            Setting::ComplexNumbers => &["complex_numbers", "imaginary"],
            Setting::DecimalPlaces => &["decimal_places", "places", "precision", "dp"],
            Setting::Notation => &["format", "formatting"],
            Setting::ExponentStep => &["exponent_step", "step", "exponents"],
            Setting::ExtendedPrefixes => &["extended_prefixes", "extended", "prefixes"],
        }
    }

    pub fn from_key(word: &str) -> Option<Setting> {
        let word = word.to_ascii_lowercase();
        Self::ALL
            .iter()
            .copied()
            .find(|s| s.key() == word || s.aliases().contains(&word.as_str()))
    }

    pub fn accepted_values(self) -> String {
        match self {
            Setting::DecimalPlaces => format!("auto | 0..{MAX_DECIMAL_PLACES}"),
            Setting::Notation => {
                "standard | power | e | si  (sci/eng also set the exponent step)".to_string()
            }
            Setting::ExponentStep => "sci | eng".to_string(),
            _ => "on | off".to_string(),
        }
    }

    pub fn command_hint(self) -> String {
        match self {
            Setting::DecimalPlaces | Setting::Notation | Setting::ExponentStep => {
                format!("{}({})", self.key(), self.accepted_values())
            }
            Setting::ExtendedPrefixes => "extprefixes(on | off)".to_string(),
            _ => format!("set({}, on | off)", self.key()),
        }
    }
}

fn parse_switch(word: &str) -> Option<bool> {
    match word.to_ascii_lowercase().as_str() {
        "on" | "true" | "yes" | "1" | "enable" | "enabled" => Some(true),
        "off" | "false" | "no" | "0" | "disable" | "disabled" => Some(false),
        _ => None,
    }
}

fn cycle<T: Copy + PartialEq>(all: &[T], current: T, forward: bool) -> T {
    let i = all.iter().position(|&x| x == current).unwrap_or(0);
    let n = all.len();
    all[if forward {
        (i + 1) % n
    } else {
        (i + n - 1) % n
    }]
}

impl Settings {
    fn switch_mut(&mut self, setting: Setting) -> Option<&mut bool> {
        Some(match setting {
            Setting::TypstRendering => &mut self.typst_rendering,
            Setting::AutoCloseBrackets => &mut self.auto_close_brackets,
            Setting::SystemClipboard => &mut self.system_clipboard,
            Setting::ComplexNumbers => &mut self.complex_numbers,
            Setting::ExtendedPrefixes => &mut self.number_format.extended_prefixes,
            _ => return None,
        })
    }

    pub fn switch(&self, setting: Setting) -> Option<bool> {
        Some(match setting {
            Setting::TypstRendering => self.typst_rendering,
            Setting::AutoCloseBrackets => self.auto_close_brackets,
            Setting::SystemClipboard => self.system_clipboard,
            Setting::ComplexNumbers => self.complex_numbers,
            Setting::ExtendedPrefixes => self.number_format.extended_prefixes,
            _ => return None,
        })
    }

    pub fn value_label(&self, setting: Setting) -> String {
        let nf = &self.number_format;
        match setting {
            Setting::DecimalPlaces => match nf.decimal_places {
                None => "auto".to_string(),
                Some(p) => p.to_string(),
            },
            Setting::Notation => nf.notation.label().to_string(),
            Setting::ExponentStep => nf.exponent_step.label().to_string(),
            _ => match self.switch(setting) {
                Some(true) => "on".to_string(),
                _ => "off".to_string(),
            },
        }
    }

    pub fn value_key(&self, setting: Setting) -> String {
        let nf = &self.number_format;
        match setting {
            Setting::Notation => nf.notation.key().to_string(),
            Setting::ExponentStep => nf.exponent_step.key().to_string(),
            _ => self.value_label(setting),
        }
    }

    pub fn is_applicable(&self, setting: Setting) -> bool {
        let notation = self.number_format.notation;
        match setting {
            Setting::ExponentStep => notation.uses_exponent_step(),
            Setting::ExtendedPrefixes => notation == Notation::SiPrefix,
            _ => true,
        }
    }

    pub fn inactive_reason(&self, setting: Setting) -> Option<&'static str> {
        if self.is_applicable(setting) {
            return None;
        }
        Some(match setting {
            Setting::ExponentStep => "only used with power/e notation",
            Setting::ExtendedPrefixes => "only used with SI prefix notation",
            _ => "not used",
        })
    }

    pub fn step(&mut self, setting: Setting, forward: bool) {
        if let Some(flag) = self.switch_mut(setting) {
            *flag = !*flag;
            return;
        }
        let nf = &mut self.number_format;
        match setting {
            Setting::DecimalPlaces => {
                nf.decimal_places = match (nf.decimal_places, forward) {
                    (None, true) => Some(0),
                    (None, false) => Some(MAX_DECIMAL_PLACES),
                    (Some(p), true) if p >= MAX_DECIMAL_PLACES => None,
                    (Some(p), true) => Some(p + 1),
                    (Some(0), false) => None,
                    (Some(p), false) => Some(p - 1),
                };
            }
            Setting::Notation => nf.notation = cycle(&Notation::ALL, nf.notation, forward),
            Setting::ExponentStep => {
                nf.exponent_step = cycle(&ExponentStep::ALL, nf.exponent_step, forward)
            }
            _ => {}
        }
    }

    pub fn set_value(&mut self, setting: Setting, word: &str) -> Result<(), String> {
        let invalid = || {
            format!(
                "Invalid value '{word}' for {}; expected {}",
                setting.key(),
                setting.accepted_values()
            )
        };
        if let Some(flag) = self.switch_mut(setting) {
            *flag = match word.to_ascii_lowercase().as_str() {
                "toggle" => !*flag,
                w => parse_switch(w).ok_or_else(invalid)?,
            };
            return Ok(());
        }
        let nf = &mut self.number_format;
        match setting {
            Setting::DecimalPlaces => {
                nf.decimal_places = match word.to_ascii_lowercase().as_str() {
                    "auto" | "off" | "none" | "all" => None,
                    w => match w.parse::<u8>() {
                        Ok(p) if p <= MAX_DECIMAL_PLACES => Some(p),
                        _ => return Err(invalid()),
                    },
                };
            }
            Setting::Notation => {
                if let Some(step) = ExponentStep::from_key(word).filter(|_| word.len() > 1) {
                    if !nf.notation.uses_exponent_step() {
                        nf.notation = Notation::Power;
                    }
                    nf.exponent_step = step;
                } else {
                    nf.notation = Notation::from_key(word).ok_or_else(invalid)?;
                }
            }
            Setting::ExponentStep => {
                nf.exponent_step = ExponentStep::from_key(word).ok_or_else(invalid)?;
            }
            _ => return Err(invalid()),
        }
        Ok(())
    }

    pub fn overview(&self) -> String {
        Setting::ALL
            .iter()
            .map(|&s| format!("{}={}", s.key(), self.value_key(s)))
            .collect::<Vec<_>>()
            .join("  ")
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
        assert_eq!(s.number_format, NumberFormat::default());
    }

    #[test]
    fn number_format_round_trips() {
        let mut s = Settings::default();
        s.set_value(Setting::Notation, "si").unwrap();
        s.set_value(Setting::DecimalPlaces, "3").unwrap();
        let json = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn switches_flip() {
        let mut s = Settings::default();
        s.step(Setting::TypstRendering, true);
        assert_eq!(s.switch(Setting::TypstRendering), Some(false));
        s.set_value(Setting::TypstRendering, "on").unwrap();
        assert!(s.typst_rendering);
        assert!(s.set_value(Setting::TypstRendering, "maybe").is_err());
    }

    #[test]
    fn choices_cycle_both_ways() {
        let mut s = Settings::default();
        s.step(Setting::DecimalPlaces, true);
        assert_eq!(s.number_format.decimal_places, Some(0));
        s.step(Setting::DecimalPlaces, false);
        s.step(Setting::DecimalPlaces, false);
        assert_eq!(s.number_format.decimal_places, Some(MAX_DECIMAL_PLACES));
        s.step(Setting::Notation, false);
        assert_eq!(s.number_format.notation, Notation::SiPrefix);
        s.step(Setting::Notation, true);
        assert_eq!(s.number_format.notation, Notation::Standard);
    }

    #[test]
    fn command_words() {
        let mut s = Settings::default();
        assert_eq!(Setting::from_key("Formatting"), Some(Setting::Notation));
        assert_eq!(Setting::from_key("precision"), Some(Setting::DecimalPlaces));
        s.set_value(Setting::Notation, "eng").unwrap();
        assert_eq!(s.number_format.notation, Notation::Power);
        assert_eq!(s.number_format.exponent_step, ExponentStep::Engineering);
        s.set_value(Setting::Notation, "e").unwrap();
        s.set_value(Setting::Notation, "sci").unwrap();
        assert_eq!(s.number_format.notation, Notation::E);
        assert!(s.set_value(Setting::DecimalPlaces, "16").is_err());
        s.set_value(Setting::DecimalPlaces, "auto").unwrap();
        assert_eq!(s.number_format.decimal_places, None);
    }

    #[test]
    fn dependent_settings_report_applicability() {
        let mut s = Settings::default();
        assert!(!s.is_applicable(Setting::ExponentStep));
        assert!(!s.is_applicable(Setting::ExtendedPrefixes));
        s.set_value(Setting::Notation, "si").unwrap();
        assert!(s.is_applicable(Setting::ExtendedPrefixes));
        assert!(s.inactive_reason(Setting::ExponentStep).is_some());
    }
}
