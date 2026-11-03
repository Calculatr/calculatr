use std::io::{Write, stdout};

use base64::{Engine, engine::general_purpose::STANDARD};

enum Backend {
    Disabled,
    System {
        native: Option<arboard::Clipboard>,
    },
    #[cfg(test)]
    Memory(Option<String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyTarget {
    Native,
    Osc52,
    Nowhere,
}

impl CopyTarget {
    pub fn label(self) -> &'static str {
        match self {
            CopyTarget::Native => "system clipboard",
            CopyTarget::Osc52 => "terminal clipboard (OSC 52)",
            CopyTarget::Nowhere => "nowhere",
        }
    }
}

pub struct Clipboard {
    backend: Backend,
}

impl Clipboard {
    pub fn disabled() -> Self {
        Self {
            backend: Backend::Disabled,
        }
    }

    pub fn system() -> Self {
        Self {
            backend: Backend::System {
                native: arboard::Clipboard::new().ok(),
            },
        }
    }

    #[cfg(test)]
    pub fn in_memory() -> Self {
        Self {
            backend: Backend::Memory(None),
        }
    }

    pub fn set(&mut self, text: &str) -> CopyTarget {
        match &mut self.backend {
            Backend::Disabled => CopyTarget::Nowhere,
            Backend::System { native } => {
                if let Some(cb) = native
                    && cb.set_text(text.to_owned()).is_ok()
                {
                    return CopyTarget::Native;
                }
                let mut out = stdout();
                let _ = out.write_all(osc52(text).as_bytes());
                let _ = out.flush();
                CopyTarget::Osc52
            }
            #[cfg(test)]
            Backend::Memory(slot) => {
                *slot = Some(text.to_owned());
                CopyTarget::Native
            }
        }
    }

    pub fn get(&mut self) -> Option<String> {
        match &mut self.backend {
            Backend::Disabled => None,
            Backend::System { native } => native.as_mut()?.get_text().ok(),
            #[cfg(test)]
            Backend::Memory(slot) => slot.clone(),
        }
        .filter(|t| !t.is_empty())
    }

    #[cfg(test)]
    pub fn set_external(&mut self, text: &str) {
        let _ = self.set(text);
    }
}

fn osc52(text: &str) -> String {
    format!("\x1b]52;c;{}\x07", STANDARD.encode(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn osc52_is_base64_wrapped() {
        assert_eq!(osc52("hi"), "\x1b]52;c;aGk=\x07");
    }

    #[test]
    fn disabled_clipboard_is_inert() {
        let mut cb = Clipboard::disabled();
        cb.set("x");
        assert_eq!(cb.get(), None);
    }
}
