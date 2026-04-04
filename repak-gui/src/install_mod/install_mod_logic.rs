pub mod archives;
pub mod iotoc;
pub mod pak_files;
pub mod patch_meshes;

use crate::install_mod::InstallableMod;
use iotoc::convert_directory_to_iostore;
use pak_files::create_repak_from_pak;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use tracing::{error, info, instrument, warn};

const MOD_NAME_SUFFIX: &str = "_9999999_P";

pub(crate) fn ensure_mod_name_suffix(name: &str) -> String {
    if name.ends_with(MOD_NAME_SUFFIX) {
        name.to_string()
    } else {
        format!("{name}{MOD_NAME_SUFFIX}")
    }
}

#[instrument(skip(mods, installed_mods_ptr, stop_thread), fields(queued_mods = mods.len(), mod_directory = ?mod_directory))]
pub fn install_mods_in_viewport(
    mods: &mut [InstallableMod],
    mod_directory: &Path,
    installed_mods_ptr: &AtomicI32,
    stop_thread: &AtomicBool,
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
            info!(mod_name = %installable_mod.mod_name, mod_path = ?installable_mod.mod_path, "Copying iostore mod");
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
                    error!(?file, error = ?e, "Unable to copy file");
                }
            }
            continue;
        }

        if installable_mod.repak {
            info!(mod_name = %installable_mod.mod_name, mod_path = ?installable_mod.mod_path, "Repacking mod");
            if let Err(e) = create_repak_from_pak(
                installable_mod,
                PathBuf::from(mod_directory),
                installed_mods_ptr,
            ) {
                error!(mod_name = %installable_mod.mod_name, error = %e, "Failed to create repak from pak");
            }
        }

        // This shit shouldnt even be possible why do I still have this in the codebase???
        if !installable_mod.repak && !installable_mod.is_dir {
            // just move files to the correct location
            info!(
                mod_name = %installable_mod.mod_name,
                mod_path = ?installable_mod.mod_path,
                "Copying pak mod directly"
            );
            std::fs::copy(
                &installable_mod.mod_path,
                mod_directory.join(format!("{normalized_mod_name}.pak")),
            )
            .unwrap();
            installed_mods_ptr.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            continue;
        }

        if installable_mod.is_dir {
            let res = convert_directory_to_iostore(
                installable_mod,
                PathBuf::from(&mod_directory),
                PathBuf::from(&installable_mod.mod_path),
                installed_mods_ptr,
            );
            if let Err(e) = res {
                error!(mod_name = %installable_mod.mod_name, mod_path = ?installable_mod.mod_path, error = %e, "Failed to convert directory");
            } else {
                info!(mod_name = %installable_mod.mod_name, "Installed directory mod");
            }
        }
    }
    // set i32 to -255 magic value to indicate mod installation is done
    AtomicI32::store(installed_mods_ptr, -255, Ordering::SeqCst);
    info!("Install worker finished");
}
