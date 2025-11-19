extern crate core;

mod file_table;
mod install_mod;
mod main_ui;
mod utils;

pub mod ios_widget;
mod utoc_utils;
mod welcome;

use crate::main_ui::RepakModManager;
use eframe::egui::{self, IconData};
use log::{info, LevelFilter};
use retoc::{action_unpack, ActionUnpack, FGuid};
use simplelog::{ColorChoice, CombinedLogger, Config, TermLogger, TerminalMode, WriteLogger};
use std::cell::LazyCell;
use std::env::args;
use std::fs::{create_dir, File};
use std::path::PathBuf;
use std::process::exit;
use std::str::FromStr;
use std::sync::atomic::AtomicI32;
use std::sync::Arc;

use crate::install_mod::install_mod_logic::iotoc::convert_directory_to_iostore;
use crate::install_mod::map_to_mods_internal;
#[cfg(target_os = "windows")]
use {rfd::MessageButtons, std::panic::PanicHookInfo};

const ICON: LazyCell<Arc<IconData>> = LazyCell::new(|| {
    let d = eframe::icon_data::from_png_bytes(include_bytes!(
        "../../repak-gui/icons/RepakLogoNonCurveFadedRed-modified.png"
    ))
    .expect("The icon data must be valid");

    Arc::new(d)
});

#[cfg(target_os = "windows")]
fn free_console() -> bool {
    unsafe { FreeConsole() == 0 }
}
#[cfg(target_os = "windows")]
fn is_console() -> bool {
    unsafe {
        let mut buffer = [0u32; 1];
        let count = GetConsoleProcessList(buffer.as_mut_ptr(), 1);
        count != 1
    }
}
#[cfg(target_os = "windows")]
#[link(name = "Kernel32")]
extern "system" {
    fn GetConsoleProcessList(processList: *mut u32, count: u32) -> u32;
    fn FreeConsole() -> i32;
}

#[cfg(target_os = "windows")]
#[cfg(not(debug_assertions))]
fn custom_panic(_info: &PanicHookInfo) -> ! {
    let msg = format!(
        "Repak has crashed. Please report this issue to the developer with the following information:\
\n\n{}\
\nAdditonally include the log file in the bug report"
        ,_info);

    let _x = rfd::MessageDialog::new()
        .set_title("Repak has crashed")
        .set_buttons(MessageButtons::Ok)
        .set_description(msg)
        .show();
    std::process::exit(1);
}

fn main() {
    #[cfg(target_os = "windows")]
    if !is_console() {
        free_console();
    }
    #[cfg(target_os = "windows")]
    #[cfg(not(debug_assertions))]
    std::panic::set_hook(Box::new(move |info| {
        custom_panic(info.into());
    }));

    /*
        Custom baked CLI utility for tobi, if the program detects a specific argument passed to it, it does not spaw GUI
    */

    let args = args().collect::<Vec<String>>();
    if args.len() > 1 {
        if args[1] == "--extract" {
            for _file in &args[2..] {
                // create a new directory for unpacking
                let path = PathBuf::from(&_file);
                let root = path.file_stem().unwrap().to_str().unwrap();
                let result_path = path.parent().unwrap().join(root);
                println!("Creating directory: {:?}", &result_path);

                let _ = create_dir(&result_path).expect("Failed to create extraction directory");
                // build an action
                let action: ActionUnpack = ActionUnpack {
                    utoc: PathBuf::from(&_file),
                    output: result_path,
                    verbose: true,
                };

                let mut config = retoc::Config {
                    container_header_version_override: None,
                    ..Default::default()
                };

                let aes_toc = retoc::AesKey::from_str(
                    "0C263D8C22DCB085894899C3A3796383E9BF9DE0CBFB08C9BF2DEF2E84F29D74",
                )
                .unwrap();

                config.aes_keys.insert(FGuid::default(), aes_toc.clone());
                let config = Arc::new(config);

                action_unpack(action, config).expect("Failed to extract");
            }
            exit(0);
        }
        if args[1] == "--pack" {
            let paths = args[2..]
                .iter()
                .map(|path| PathBuf::from_str(path).unwrap())
                .filter(|path| path.is_dir())
                .collect::<Vec<_>>();
            let installable_mods = map_to_mods_internal(&paths);
            for (i, installable) in installable_mods.iter().enumerate() {
                let mod_dir = paths[i].parent().unwrap();
                let count = AtomicI32::new(0);
                convert_directory_to_iostore(
                    &installable,
                    mod_dir.to_path_buf(),
                    paths[i].clone(),
                    &count,
                )
                .expect("Failed to convert directory");
            }
            exit(0);
        }
    }

    // This forces repak gui to use the XWAYLAND backend instead of the wayland as wayland backend is half baked as shit
    // and doesnt support icons and drag and drop
    unsafe {
        #[cfg(target_os = "linux")]
        std::env::set_var("WINIT_UNIX_BACKEND", "x11");
        std::env::remove_var("WAYLAND_DISPLAY");
    }

    let log_file = File::create("latest.log").expect("Failed to create log file");
    let level_filter = if cfg!(debug_assertions) {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };

    CombinedLogger::init(vec![
        TermLogger::new(
            level_filter,
            Config::default(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        ),
        WriteLogger::new(LevelFilter::Info, Config::default(), log_file),
    ])
    .expect("Failed to initialize logger");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1366.0, 768.0])
            .with_min_inner_size([1100.0, 650.])
            .with_drag_and_drop(true)
            .with_icon(ICON.clone()),
        ..Default::default()
    };

    eframe::run_native(
        "Repak GUI",
        options,
        Box::new(|cc| {
            cc.egui_ctx
                .style_mut(|style| style.visuals.dark_mode = true);
            Ok(Box::new(
                RepakModManager::load(cc).expect("Unable to load config"),
            ))
        }),
    )
    .expect("Unable to spawn windows");
}
