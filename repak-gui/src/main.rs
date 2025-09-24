extern crate core;

mod main_ui;
mod file_table;
mod install_mod;
mod utils;

pub mod ios_widget;
mod utoc_utils;
mod welcome;

use eframe::egui::{
    self, IconData,
};
use log::LevelFilter;
use simplelog::{ColorChoice, CombinedLogger, Config, TermLogger, TerminalMode, WriteLogger};
use std::cell::LazyCell;
use std::fs::File;
use std::sync::Arc;

use crate::main_ui::RepakModManager;

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
