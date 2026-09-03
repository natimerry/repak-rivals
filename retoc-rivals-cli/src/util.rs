use repak::utils::AesKey as PakAesKey;
use std::str::FromStr;

use crate::cli::CompressionArg;
use crate::{MOD_NAME_SUFFIX, RIVALS_AES_KEY};

pub fn pak_aes_key() -> Result<PakAesKey, String> {
    PakAesKey::from_str(RIVALS_AES_KEY).map_err(|e| format!("Failed to parse AES key: {e}"))
}

pub fn ensure_mod_name_suffix(name: &str) -> String {
    if name.ends_with(MOD_NAME_SUFFIX) {
        name.to_string()
    } else {
        format!("{name}{MOD_NAME_SUFFIX}")
    }
}

pub fn retoc_compression(value: CompressionArg) -> Option<retoc::compression::CompressionMethod> {
    match value {
        CompressionArg::None => None,
        CompressionArg::Zlib => Some(retoc::compression::CompressionMethod::Zlib),
        CompressionArg::Zstd => Some(retoc::compression::CompressionMethod::Zstd),
        CompressionArg::Lz4 => Some(retoc::compression::CompressionMethod::LZ4),
        CompressionArg::Oodle => Some(retoc::compression::CompressionMethod::Oodle),
    }
}

pub fn parse_path_hash_seed(path_hash_seed: &str) -> Result<u64, String> {
    u64::from_str_radix(path_hash_seed.trim_start_matches("0x"), 16)
        .or_else(|_| path_hash_seed.parse())
        .map_err(|e| format!("Failed to parse path hash seed '{path_hash_seed}': {e}"))
}
