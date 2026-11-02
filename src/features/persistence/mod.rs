use crate::features::core::{Environment, RenderMode};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
pub struct SavedState {
    pub render_mode: RenderMode,
    pub history: Vec<String>,
    pub env: Environment,
}

pub fn get_config_path() -> Option<PathBuf> {
    let dir = directories::ProjectDirs::from("com", "calculatr", "Calculatr")?;
    let path = dir.config_dir().join("config.json");
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    Some(path)
}

pub fn load_state() -> Option<SavedState> {
    let path = get_config_path()?;
    let data = fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

pub fn save_state(render_mode: RenderMode, history: &[String], env: &Environment) {
    if let Some(path) = get_config_path() {
        let state = SavedState {
            render_mode,
            history: history.to_vec(),
            env: env.clone(),
        };
        if let Ok(data) = serde_json::to_string_pretty(&state) {
            let _ = fs::write(path, data);
        }
    }
}
