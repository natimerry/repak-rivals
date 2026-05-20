pub mod archives;
pub mod iotoc;
pub mod pak_files;
pub mod patch_meshes;

use crate::install_mod::InstallableMod;
use iotoc::convert_directory_to_iostore;
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
            // THIS IS CURRENTLY BROKEN AS RETOC ISNT PROVIDED GAME GLOBAL CHUNKDATA FILE
            // WHEN THE UI IS UPDATED WE WILL ADD THIS CAPABILITY

            // if installable_mod.repak && chunkdir.is_some() {
            //     let chunkdir = chunkdir.clone().unwrap();
            //     info!(mod_name = %installable_mod.mod_name, mod_path = ?installable_mod.mod_path, chunkdir = ?chunkdir, "Repacking IoStore mod with current settings");
            //     // unpack to temp dir, then convert_directory_to_iostore
            //     if let Err(e) = repack_iostore_mod(
            //         installable_mod,
            //         PathBuf::from(mod_directory),
            //         chunkdir.into(),
            //         installed_mods_ptr,
            //     ) {
            //         error!(mod_name = %installable_mod.mod_name, error = %e, "Failed to repack IoStore mod");
            //     }
            //     continue;
            // } else {
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
            // }
        }

        if installable_mod.repak {
            info!(mod_name = %installable_mod.mod_name, is_repak = true, "Repacking mod");
            if let Err(e) = create_repak_from_pak(
                installable_mod,
                PathBuf::from(mod_directory),
                installed_mods_ptr.clone(),
                kawaii_physics_usmap,
            ) {
                error!(mod_name = %installable_mod.mod_name, error = %e, "Failed to create repak from pak");
            }
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
