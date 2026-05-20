pub mod archives;
pub mod iotoc;
pub mod pak_files;
pub mod patch_meshes;

use crate::install_mod::InstallableMod;
use iotoc::{convert_directory_to_iostore, to_legacy_uasset_fast};
use pak_files::create_repak_from_pak;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;
use tracing::{error, info, instrument, warn};

const MOD_NAME_SUFFIX: &str = "_9999999_P";

pub(crate) fn ensure_mod_name_suffix(name: &str) -> String {
    if name.ends_with(MOD_NAME_SUFFIX) {
        name.to_string()
    } else {
        format!("{name}{MOD_NAME_SUFFIX}")
    }
}

#[cfg(all(windows, not(debug_assertions)))]
fn show_to_legacy_console() {
    crate::ensure_console();
    crate::redirect_stdio();
}

#[cfg(all(windows, not(debug_assertions)))]
fn close_to_legacy_console() {
    // Keep the release progress console alive for the process. Closing between
    // per-mod to-legacy calls causes Windows to allocate a new console each time.
}

fn backup_existing_mod_files(
    mod_directory: &Path,
    normalized_mod_name: &str,
) -> std::io::Result<()> {
    for ext in ["pak", "utoc", "ucas"] {
        let path = mod_directory.join(format!("{normalized_mod_name}.{ext}"));
        if !path.exists() {
            continue;
        }

        let mut backup = mod_directory.join(format!("{normalized_mod_name}.{ext}.rebak"));
        let mut suffix = 1usize;
        while backup.exists() {
            backup = mod_directory.join(format!("{normalized_mod_name}.{ext}.{suffix}.rebak"));
            suffix += 1;
        }
        std::fs::rename(&path, &backup)?;
    }

    Ok(())
}

fn copy_fixed_iostore_files(
    from_dir: &Path,
    mod_directory: &Path,
    normalized_mod_name: &str,
) -> std::io::Result<()> {
    for ext in ["pak", "utoc", "ucas"] {
        let file_name = format!("{normalized_mod_name}.{ext}");
        let src = from_dir.join(&file_name);
        if src.exists() {
            std::fs::copy(src, mod_directory.join(file_name))?;
        }
    }

    Ok(())
}

fn repack_iostore_via_fast_extract(
    installable_mod: &InstallableMod,
    output_mod_dir: &Path,
    source_mods_dir: &Path,
    installed_mods_ptr: Arc<AtomicI32>,
    chunkdir: &Option<PathBuf>,
    kawaii_physics_usmap: &Option<PathBuf>,
    kawaii_porter: bool,
) -> Result<(), repak::Error> {
    let chunkdir = chunkdir.clone().ok_or_else(|| {
        repak::Error::Io(std::io::Error::other(
            "Cannot repack IoStore mod without detected game Paks directory",
        ))
    })?;

    let extract_temp = tempfile::tempdir().map_err(repak::Error::Io)?;
    #[cfg(all(windows, not(debug_assertions)))]
    show_to_legacy_console();
    let extract_result = to_legacy_uasset_fast(
        installable_mod.mod_path.clone(),
        extract_temp.path().to_path_buf(),
        source_mods_dir.to_path_buf(),
        chunkdir,
    );
    #[cfg(all(windows, not(debug_assertions)))]
    close_to_legacy_console();
    extract_result?;

    let extracted_dir = extract_temp.path().join(
        installable_mod
            .mod_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or(&installable_mod.mod_name),
    );
    let mut fixed_mod = installable_mod.clone();
    fixed_mod.is_dir = true;
    fixed_mod.iostore = false;
    fixed_mod.repak = false;
    fixed_mod.kawaii_porter = kawaii_porter;
    fixed_mod.mod_path = extracted_dir.clone();
    fixed_mod.reader = None;

    convert_directory_to_iostore(
        &fixed_mod,
        output_mod_dir.to_path_buf(),
        extracted_dir,
        installed_mods_ptr,
        kawaii_physics_usmap.clone(),
        kawaii_porter,
    )?;

    Ok(())
}

pub fn fix_installed_iostore_kawaii_physics(
    installable_mod: &InstallableMod,
    mod_directory: &Path,
    installed_mods_ptr: Arc<AtomicI32>,
    chunkdir: &Option<PathBuf>,
    kawaii_physics_usmap: &Option<PathBuf>,
) -> Result<(), repak::Error> {
    let output_temp = tempfile::tempdir().map_err(repak::Error::Io)?;
    repack_iostore_via_fast_extract(
        installable_mod,
        output_temp.path(),
        mod_directory,
        installed_mods_ptr,
        chunkdir,
        kawaii_physics_usmap,
        true,
    )?;

    backup_existing_mod_files(mod_directory, &installable_mod.mod_name)
        .map_err(repak::Error::Io)?;
    copy_fixed_iostore_files(output_temp.path(), mod_directory, &installable_mod.mod_name)
        .map_err(repak::Error::Io)?;

    Ok(())
}

#[instrument(
    skip_all,
    fields(
        queued_mods = mods.len(),
        has_chunkdir = chunkdir.is_some(),
        has_kawaii_physics_usmap = kawaii_physics_usmap.is_some(),
        mod_directory_exists = mod_directory.exists()
    )
)]
pub fn install_mods_in_viewport(
    mods: &mut [InstallableMod],
    mod_directory: &Path,
    installed_mods_ptr: Arc<AtomicI32>,
    stop_thread: &AtomicBool,
    chunkdir: &Option<PathBuf>,
    kawaii_physics_usmap: &Option<PathBuf>,
) {
    info!("Installing queued mods");
    for installable_mod in mods.iter_mut() {
        if !installable_mod.enabled {
            info!(mod_name = %installable_mod.mod_name, "Skipping disabled mod");
            continue;
        }

        if stop_thread.load(Ordering::SeqCst) {
            warn!(mod_name = %installable_mod.mod_name, "Stopping install worker");
            break;
        }

        let normalized_mod_name = ensure_mod_name_suffix(&installable_mod.mod_name);
        installable_mod.mod_name = normalized_mod_name.clone();

        if installable_mod.iostore {
            if installable_mod.repak {
                let source_mods_dir = installable_mod
                    .mod_path
                    .parent()
                    .unwrap_or_else(|| Path::new(""));
                info!(mod_name = %installable_mod.mod_name, "Repacking IoStore mod via fast extraction");
                if let Err(e) = repack_iostore_via_fast_extract(
                    installable_mod,
                    mod_directory,
                    source_mods_dir,
                    installed_mods_ptr.clone(),
                    chunkdir,
                    kawaii_physics_usmap,
                    installable_mod.kawaii_porter,
                ) {
                    error!(mod_name = %installable_mod.mod_name, error = %e, "Failed to repack IoStore mod");
                }
                continue;
            }

            info!(mod_name = %installable_mod.mod_name, is_iostore = true, "Copying iostore mod");
            // copy the iostore files
            let pak_path = installable_mod.mod_path.with_extension("pak");
            let utoc_path = installable_mod.mod_path.with_extension("utoc");
            let ucas_path = installable_mod.mod_path.with_extension("ucas");

            let files_to_copy = vec![
                (pak_path, format!("{normalized_mod_name}.pak")),
                (utoc_path, format!("{normalized_mod_name}.utoc")),
                (ucas_path, format!("{normalized_mod_name}.ucas")),
            ];

            for (file, target_name) in files_to_copy {
                if let Err(e) = std::fs::copy(&file, mod_directory.join(target_name)) {
                    error!(
                        file_name = %file.file_name().and_then(|name| name.to_str()).unwrap_or("<unknown>"),
                        error = ?e,
                        "Unable to copy file"
                    );
                }
            }
            installed_mods_ptr.fetch_add(
                installable_mod.total_files.max(1).min(i32::MAX as usize) as i32,
                Ordering::SeqCst,
            );
            continue;
        }

        if installable_mod.repak {
            info!(mod_name = %installable_mod.mod_name, is_repak = true, "Repacking mod");
            if let Err(e) = create_repak_from_pak(
                installable_mod,
                PathBuf::from(mod_directory),
                installed_mods_ptr.clone(),
                kawaii_physics_usmap,
                installable_mod.kawaii_porter,
            ) {
                error!(mod_name = %installable_mod.mod_name, error = %e, "Failed to create repak from pak");
            }
            continue;
        }

        // This shit shouldnt even be possible why do I still have this in the codebase???
        if !installable_mod.repak && !installable_mod.is_dir {
            // just move files to the correct location
            info!(
                mod_name = %installable_mod.mod_name,
                source_is_dir = installable_mod.mod_path.is_dir(),
                "Copying pak mod directly"
            );
            std::fs::copy(
                &installable_mod.mod_path,
                mod_directory.join(format!("{normalized_mod_name}.pak")),
            )
            .unwrap();
            installed_mods_ptr.fetch_add(
                installable_mod.total_files.max(1).min(i32::MAX as usize) as i32,
                Ordering::SeqCst,
            );
            continue;
        }

        if installable_mod.is_dir {
            let res = convert_directory_to_iostore(
                installable_mod,
                PathBuf::from(&mod_directory),
                PathBuf::from(&installable_mod.mod_path),
                installed_mods_ptr.clone(),
                kawaii_physics_usmap.clone(),
                installable_mod.kawaii_porter,
            );
            if let Err(e) = res {
                error!(
                    mod_name = %installable_mod.mod_name,
                    source_is_dir = installable_mod.mod_path.is_dir(),
                    error = %e,
                    "Failed to convert directory"
                );
            } else {
                info!(mod_name = %installable_mod.mod_name, "Installed directory mod");
            }
        }
    }
    // set i32 to -255 magic value to indicate mod installation is done
    AtomicI32::store(&installed_mods_ptr, -255, Ordering::SeqCst);
    info!("Install worker finished");
}
