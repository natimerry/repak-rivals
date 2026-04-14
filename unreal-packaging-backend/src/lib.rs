use anyhow::{Context, Result};
use retoc::{action_to_zen, action_unpack, ActionToZen, ActionUnpack, Config, EngineVersion, FGuid};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

pub const DEFAULT_AES_KEY_HEX: &str =
    "0C263D8C22DCB085894899C3A3796383E9BF9DE0CBFB08C9BF2DEF2E84F29D74";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnrealEngineVersion {
    UE5_3,
}

#[derive(Debug, Clone)]
pub struct ExtractUtocRequest {
    pub input_utoc: PathBuf,
    pub output_dir: PathBuf,
    pub verbose: bool,
}

#[derive(Debug, Clone)]
pub struct PackDirectoryRequest {
    pub input_dir: PathBuf,
    pub output_utoc: PathBuf,
    pub engine_version: UnrealEngineVersion,
}

pub trait UnrealPackagingBackend: Send + Sync {
    fn name(&self) -> &'static str;
    fn extract_utoc(&self, request: &ExtractUtocRequest) -> Result<()>;
    fn pack_directory_to_iostore(&self, request: &PackDirectoryRequest) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct RetocBackend {
    aes_key_hex: String,
}

impl Default for RetocBackend {
    fn default() -> Self {
        Self {
            aes_key_hex: DEFAULT_AES_KEY_HEX.to_string(),
        }
    }
}

impl RetocBackend {
    pub fn new(aes_key_hex: impl Into<String>) -> Self {
        Self {
            aes_key_hex: aes_key_hex.into(),
        }
    }

    fn build_config(&self) -> Result<Arc<Config>> {
        let mut config = Config {
            container_header_version_override: None,
            ..Default::default()
        };
        let aes_key = retoc::AesKey::from_str(&self.aes_key_hex).context("invalid AES key")?;
        config.aes_keys.insert(FGuid::default(), aes_key);
        Ok(Arc::new(config))
    }
}

impl UnrealPackagingBackend for RetocBackend {
    fn name(&self) -> &'static str {
        "retoc"
    }

    fn extract_utoc(&self, request: &ExtractUtocRequest) -> Result<()> {
        ensure_parent_exists(&request.output_dir)?;
        let action = ActionUnpack {
            utoc: request.input_utoc.clone(),
            output: request.output_dir.clone(),
            verbose: request.verbose,
        };
        action_unpack(action, self.build_config()?)?;
        Ok(())
    }

    fn pack_directory_to_iostore(&self, request: &PackDirectoryRequest) -> Result<()> {
        ensure_parent_exists(&request.output_utoc)?;
        let action = ActionToZen::new(
            request.input_dir.clone(),
            request.output_utoc.clone(),
            match request.engine_version {
                UnrealEngineVersion::UE5_3 => EngineVersion::UE5_3,
            },
        );
        action_to_zen(action, self.build_config()?)?;
        Ok(())
    }
}

fn ensure_parent_exists(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    Ok(())
}
