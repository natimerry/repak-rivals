use retoc::{Config, FGuid};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(serde::Deserialize)]
pub struct CliState {
    pub game_path: PathBuf,
    pub game_chunk_path: Option<PathBuf>,
    pub kawaii_physics_usmap: Option<PathBuf>,
}

pub fn retoc_config(aes_key: retoc::AesKey) -> Arc<Config> {
    let mut config = Config {
        container_header_version_override: None,
        ..Default::default()
    };
    config.aes_keys.insert(FGuid::default(), aes_key);
    Arc::new(config)
}

pub fn read_saved_state() -> Result<CliState, String> {
    let config_path = cli_config_paths()
        .into_iter()
        .find(|path| path.exists())
        .ok_or_else(|| "Could not find saved repak-rivals state".to_string())?;
    let state = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read {}: {e}", config_path.display()))?;
    serde_json::from_str(&state)
        .map_err(|e| format!("Failed to parse {}: {e}", config_path.display()))
}

fn cli_config_paths() -> [PathBuf; 2] {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("repak_manager");
    [dir.join("repak_mod_manager.json"), dir.join("state.json")]
}
