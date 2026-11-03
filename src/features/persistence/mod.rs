use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::features::core::{Environment, RenderMode};
use crate::features::history::{HistoryEntry, MAX_SAVED_ENTRIES};
use crate::features::settings::Settings;

#[derive(Serialize, Deserialize)]
pub struct SavedState {
    pub render_mode: RenderMode,
    pub history: Vec<HistoryEntry>,
    pub env: Environment,
}

fn config_dir() -> Option<PathBuf> {
    let dirs = directories::ProjectDirs::from("com", "calculatr", "Calculatr")?;
    let dir = dirs.config_dir().to_path_buf();
    fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    let data = fs::read_to_string(path).ok()?;
    match serde_json::from_str(&data) {
        Ok(value) => Some(value),
        Err(_) => {
            let _ = fs::rename(path, path.with_extension("json.bak"));
            None
        }
    }
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    let data = serde_json::to_string_pretty(value).map_err(io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, data)?;
    fs::rename(&tmp, path)
}

pub fn load_settings() -> Settings {
    config_dir()
        .and_then(|d| read_json(&d.join("settings.json")))
        .unwrap_or_default()
}

pub fn save_settings(settings: &Settings) -> io::Result<()> {
    let dir = config_dir().ok_or_else(|| io::Error::other("no config directory"))?;
    write_json(&dir.join("settings.json"), settings)
}

pub fn load_state() -> Option<SavedState> {
    read_json(&config_dir()?.join("state.json"))
}

pub fn save_state(render_mode: RenderMode, history: &[HistoryEntry], env: &Environment) {
    let Some(dir) = config_dir() else { return };
    let skip = history.len().saturating_sub(MAX_SAVED_ENTRIES);
    let state = SavedState {
        render_mode,
        history: history[skip..].to_vec(),
        env: env.clone(),
    };
    let _ = write_json(&dir.join("state.json"), &state);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::core::evaluate;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("calculatr-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn state_roundtrip_keeps_exact_numbers_and_equations() {
        let mut env = Environment::new();
        evaluate(&mut env, "x := 0.1", RenderMode::Symbolic);
        evaluate(&mut env, "e1 := 3y = 2", RenderMode::Symbolic);
        let state = SavedState {
            render_mode: RenderMode::Symbolic,
            history: vec![],
            env,
        };
        let dir = temp_dir("state");
        let path = dir.join("state.json");
        write_json(&path, &state).unwrap();
        let back: SavedState = read_json(&path).unwrap();
        assert_eq!(back.render_mode, RenderMode::Symbolic);
        assert_eq!(back.env.get_expr("x"), state.env.get_expr("x"));
        assert!(back.env.get_expr("e1").is_some());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn corrupt_file_is_backed_up_not_overwritten() {
        let dir = temp_dir("corrupt");
        let path = dir.join("state.json");
        fs::write(&path, "{ not json").unwrap();
        assert!(read_json::<SavedState>(&path).is_none());
        assert!(!path.exists());
        assert!(dir.join("state.json.bak").exists());
        let _ = fs::remove_dir_all(dir);
    }
}
