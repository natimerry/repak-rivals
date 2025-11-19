pub mod archives;
pub mod iotoc;
pub mod pak_files;
pub mod patch_meshes;

use crate::install_mod::InstallableMod;
use iotoc::convert_directory_to_iostore;
use log::{error, info, warn};
use pak_files::create_repak_from_pak;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

pub fn install_mods_in_viewport(
    mods: &mut [InstallableMod],
    mod_directory: &Path,
    installed_mods_ptr: &AtomicI32,
    stop_thread: &AtomicBool,
) {
    for installable_mod in mods.iter_mut() {
        
        if !installable_mod.enabled{
            continue;
        }
        
        
        if stop_thread.load(Ordering::SeqCst) {
            warn!("Stopping thread");
            break;
        }

        if installable_mod.iostore {
            // copy the iostore files
            let pak_path = installable_mod.mod_path.with_extension("pak");
            let utoc_path = installable_mod.mod_path.with_extension("utoc");
            let ucas_path = installable_mod.mod_path.with_extension("ucas");

            let files_to_copy = vec![pak_path, utoc_path, ucas_path];

            for file in files_to_copy {
                if let Err(e) = std::fs::copy(&file, mod_directory.join(file.file_name().unwrap())) {
                    error!("Unable to copy file {:?}: {:?}",file, e);
                }
            }
            continue;
        }

        if installable_mod.repak {
            if let Err(e) = create_repak_from_pak(
                installable_mod,
                PathBuf::from(mod_directory),
                installed_mods_ptr,
            ) {
                error!("Failed to create repak from pak: {}", e);
            }
        }

        // This shit shouldnt even be possible why do I still have this in the codebase???
        if !installable_mod.repak && !installable_mod.is_dir {
            // just move files to the correct location
            info!(
                "Copying mod instead of repacking: {}",
                installable_mod.mod_name
            );
            std::fs::copy(
                &installable_mod.mod_path,
                mod_directory.join(format!("{}.pak", &installable_mod.mod_name)),
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
                error!("Failed to create repak from pak: {}", e);
            } else {
                info!("Installed mod: {}", installable_mod.mod_name);
            }
        }
    }
    // set i32 to -255 magic value to indicate mod installation is done
    AtomicI32::store(installed_mods_ptr, -255, Ordering::SeqCst);
}
