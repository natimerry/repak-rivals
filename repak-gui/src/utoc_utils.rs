use repak::PakReader;
use retoc::{action_manifest, ActionManifest, Config, FGuid};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

const UTOC_MAGIC: &[u8; 16] = b"-==--==--==--==-";
const CONTAINER_FLAGS_OFFSET: u64 = 80;
const CONTAINER_FLAG_ENCRYPTED: u8 = 0b0010;

pub fn read_utoc(
    utoc_path: &Path,
    pak_reader: &PakReader,
    pak_path: &Path,
) -> Vec<crate::file_table::FileEntry> {
    let action_mn = ActionManifest::new(PathBuf::from(utoc_path));
    let mut config = Config {
        container_header_version_override: None,
        ..Default::default()
    };

    let aes_toc = retoc::AesKey::from_str(
        "0C263D8C22DCB085894899C3A3796383E9BF9DE0CBFB08C9BF2DEF2E84F29D74",
    )
    .expect("Failed to parse AES key");

    config.aes_keys.insert(FGuid::default(), aes_toc.clone());
    let config = Arc::new(config);

    let ops = action_manifest(action_mn, config).expect("Failed to read utoc");
    ops.oplog
        .entries
        .iter()
        .map(|entry| {
            let name = entry.packagestoreentry.packagename.clone();
            crate::file_table::FileEntry {
                file_path: name,
                pak_path: PathBuf::from(pak_path),
                pak_reader: pak_reader.clone(),
                // entry: pak_reader.get_file_entry(entry).unwrap(),
                compressed: "Unavailable".to_string(),
                uncompressed: "Unavailable".to_string(),
                offset: "Unavailable".to_string(),
                bulkdata: Some(entry.bulkdata.len()),
                package_data: Some(entry.packagedata.len()),
            }
        })
        .collect::<Vec<_>>()
}

pub fn read_utoc_package_names(utoc_path: &Path) -> Result<Vec<String>, String> {
    let action_mn = ActionManifest::new(PathBuf::from(utoc_path));
    let mut config = Config {
        container_header_version_override: None,
        ..Default::default()
    };

    let aes_toc = retoc::AesKey::from_str(
        "0C263D8C22DCB085894899C3A3796383E9BF9DE0CBFB08C9BF2DEF2E84F29D74",
    )
    .map_err(|e| format!("Failed to parse AES key: {e}"))?;

    config.aes_keys.insert(FGuid::default(), aes_toc.clone());
    let config = Arc::new(config);

    let ops =
        action_manifest(action_mn, config).map_err(|e| format!("Failed to read utoc manifest: {e}"))?;
    Ok(ops
        .oplog
        .entries
        .iter()
        .map(|entry| entry.packagestoreentry.packagename.clone())
        .collect())
}

pub fn is_iostore_obfuscated(utoc_path: &Path) -> Result<bool, String> {
    let mut file = std::fs::File::open(utoc_path)
        .map_err(|e| format!("Failed to open {}: {e}", utoc_path.display()))?;

    let mut magic = [0u8; 16];
    file.read_exact(&mut magic).map_err(|e| {
        format!(
            "Failed to read .utoc magic from {}: {e}",
            utoc_path.display()
        )
    })?;

    if &magic != UTOC_MAGIC {
        return Err(format!("Unrecognized .utoc file: {}", utoc_path.display()));
    }

    file.seek(SeekFrom::Start(CONTAINER_FLAGS_OFFSET))
        .map_err(|e| format!("Failed to seek .utoc flags in {}: {e}", utoc_path.display()))?;

    let mut flags = [0u8; 1];
    file.read_exact(&mut flags).map_err(|e| {
        format!(
            "Failed to read .utoc flags from {}: {e}",
            utoc_path.display()
        )
    })?;

    Ok(flags[0] & CONTAINER_FLAG_ENCRYPTED != 0)
}
