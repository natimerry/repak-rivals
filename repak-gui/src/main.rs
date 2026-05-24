#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

extern crate core;

mod file_table;
mod install_mod;
mod install_terminal;
mod launch_game;
mod main_ui;
mod updater;
mod utils;

pub mod ios_widget;
mod utoc_utils;
mod welcome;
use crate::install_mod::install_mod_logic::install_mods_in_viewport;
use crate::install_mod::install_mod_logic::iotoc::{
    convert_directory_to_iostore, to_legacy_uasset_fast_batch,
};
use crate::install_mod::map_to_mods_internal;
use crate::main_ui::RepakModManager;
use crate::utils::SkinEntry;
use eframe::egui::{self, IconData};
use retoc::{action_unpack, ActionUnpack, FGuid};
use std::cell::LazyCell;
use std::collections::HashMap;
use std::env::args;
use std::fs::{self, create_dir, File};
use std::io::BufWriter;
use std::io::Write;
use std::path::PathBuf;
use std::process::exit;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicI32};
use std::sync::Arc;
use std::thread;
use tracing::{debug, info, instrument};
use tracing_subscriber::filter::{LevelFilter, Targets};
use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;
use walkdir::WalkDir;

#[cfg(target_os = "windows")]
#[cfg(not(debug_assertions))]
use {rfd::MessageButtons, std::panic::PanicHookInfo};

const ICON: LazyCell<Arc<IconData>> = LazyCell::new(|| {
    let d = eframe::icon_data::from_png_bytes(include_bytes!(
        "../../repak-gui/icons/RepakLogoNonCurveFadedRed-modified.png"
    ))
    .expect("The icon data must be valid");

    Arc::new(d)
});

#[cfg(target_os = "windows")]
#[allow(dead_code)]
fn free_console() -> bool {
    #[cfg(not(debug_assertions))]
    {
        if !CONSOLE_ALLOCATED_BY_US.swap(false, std::sync::atomic::Ordering::SeqCst) {
            return false;
        }
        CONSOLE_READY.store(false, std::sync::atomic::Ordering::SeqCst);
    }

    unsafe { FreeConsole() != 0 }
}

#[cfg(target_os = "windows")]
fn detach_startup_console() -> bool {
    #[cfg(not(debug_assertions))]
    {
        CONSOLE_READY.store(false, std::sync::atomic::Ordering::SeqCst);
        CONSOLE_ALLOCATED_BY_US.store(false, std::sync::atomic::Ordering::SeqCst);
    }

    unsafe { FreeConsole() != 0 }
}

#[cfg(windows)]
pub mod win_console {
    #[link(name = "kernel32")]
    extern "system" {
        pub fn AllocConsole() -> i32;
        pub fn AttachConsole(dwProcessId: u32) -> i32;
        pub fn SetStdHandle(nStdHandle: u32, hHandle: *mut core::ffi::c_void) -> i32;
        pub fn GetStdHandle(nStdHandle: u32) -> *mut core::ffi::c_void;
        pub fn GetConsoleWindow() -> *mut core::ffi::c_void;
        pub fn CreateFileA(
            lpFileName: *const u8,
            dwDesiredAccess: u32,
            dwShareMode: u32,
            lpSecurityAttributes: *mut core::ffi::c_void,
            dwCreationDisposition: u32,
            dwFlagsAndAttributes: u32,
            hTemplateFile: *mut core::ffi::c_void,
        ) -> *mut core::ffi::c_void;
    }

    pub const ATTACH_PARENT_PROCESS: u32 = 0xFFFF_FFFF;

    pub const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
    pub const STD_ERROR_HANDLE: u32 = -12i32 as u32;

    pub const FILE_GENERIC_WRITE: u32 = 0x40000000;
    pub const FILE_SHARE_READ: u32 = 0x00000001;
    pub const FILE_SHARE_WRITE: u32 = 0x00000002;

    pub const OPEN_EXISTING: u32 = 3;
}

#[cfg(all(windows, not(debug_assertions)))]
static CONSOLE_READY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
#[cfg(all(windows, not(debug_assertions)))]
static CONSOLE_ALLOCATED_BY_US: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[cfg(windows)]
pub fn ensure_console() {
    use win_console::*;
    #[cfg(not(debug_assertions))]
    if CONSOLE_READY.load(std::sync::atomic::Ordering::SeqCst) {
        redirect_stdio();
        return;
    }

    unsafe {
        if !GetConsoleWindow().is_null() {
            #[cfg(not(debug_assertions))]
            CONSOLE_READY.store(true, std::sync::atomic::Ordering::SeqCst);
            redirect_stdio();
            return;
        }

        if AttachConsole(ATTACH_PARENT_PROCESS) != 0 {
            #[cfg(not(debug_assertions))]
            CONSOLE_READY.store(true, std::sync::atomic::Ordering::SeqCst);
            redirect_stdio();
            return;
        }

        if AllocConsole() != 0 {
            #[cfg(not(debug_assertions))]
            {
                CONSOLE_READY.store(true, std::sync::atomic::Ordering::SeqCst);
                CONSOLE_ALLOCATED_BY_US.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            redirect_stdio();
        }
    }
}

#[cfg(windows)]
pub fn redirect_stdio() {
    use std::ptr;
    use win_console::*;

    unsafe {
        let name = b"CONOUT$\0";

        let handle = CreateFileA(
            name.as_ptr(),
            FILE_GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            ptr::null_mut(),
            OPEN_EXISTING,
            0,
            ptr::null_mut(),
        );

        if !handle.is_null() {
            SetStdHandle(STD_OUTPUT_HANDLE, handle);
            SetStdHandle(STD_ERROR_HANDLE, handle);
        }

        // Force Rust stdio to rebind
        let _ = std::io::stdout();
        let _ = std::io::stderr();
    }
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
pub fn has_attached_console() -> bool {
    unsafe { !win_console::GetConsoleWindow().is_null() }
}

#[cfg(not(target_os = "windows"))]
pub fn has_attached_console() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal() || std::io::stderr().is_terminal()
}
#[cfg(target_os = "windows")]
#[link(name = "Kernel32")]
extern "system" {
    fn GetConsoleProcessList(process_list: *mut u32, count: u32) -> u32;
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

#[instrument(name = "fetch_skins_in_background")]
pub fn fetch_skins_in_background(
) -> thread::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>> {
    info!("Fetching updated skin data");
    thread::spawn(|| {
        let client = reqwest::blocking::Client::new();
        let response = client
            .get("https://raw.githubusercontent.com/donutman07/MarvelRivalsCharacterIDs/refs/heads/main/MarvelRivalsCharacterIDs.md")
            .send()?
            .error_for_status()?;
        let body = response.text()?;
        debug!("Received markdown response ({} bytes)", body.len());

        let skins = parse_markdown_to_skin_entries(&body);
        debug!("Parsed {} skin entries", skins.len());

        let json = serde_json::to_string_pretty(&skins)?;
        fs::create_dir_all("data")?;
        fs::write("data/character_data.json", json)?;
        Ok(())
    })
}

#[instrument(skip(markdown))]
fn parse_markdown_to_skin_entries(markdown: &str) -> Vec<SkinEntry> {
    let mut entries = Vec::new();
    let mut current_char_id: Option<String> = None;
    let mut current_char_name: Option<String> = None;

    for line in markdown.lines() {
        let line = line.trim();
        if !line.starts_with('|') {
            continue;
        }

        if line.contains("NAME") || line.contains(":--:") {
            continue;
        }

        let cols: Vec<&str> = line.split('|').map(|s| s.trim()).collect();

        if cols.len() < 5 {
            continue;
        }

        let id_col = cols[1];
        let name_col = cols[2];
        let skin_id_col = cols[3];
        let skin_name_col = cols[4];

        // Update current character if this row has an ID
        if !id_col.is_empty() && id_col != "????" {
            // Validate it's a numeric character ID
            if id_col.chars().all(|c| c.is_ascii_digit()) {
                let name = name_col.to_string();
                // Skip placeholder/old/bot entries that would override real characters
                let is_placeholder = name.contains("(Old)")
                    || name.contains("Old)")
                    || name_col.is_empty()
                    || name.ends_with("Bot")
                    || name.contains("Bot (")
                    || name.starts_with("Zombie")
                    || name.starts_with("No Data")
                    || name.contains("Mislabeled");

                if is_placeholder {
                    current_char_id = None;
                    current_char_name = None;
                    continue; // don't update current character context
                }

                current_char_id = Some(id_col.to_string());
                current_char_name = Some(name_col.to_string());

                // Generate default skin entry: 4-digit char ID + "001"
                let default_skin_id = format!("{}001", id_col);
                let default_skin_name = "(Default)".to_string();
                entries.push(SkinEntry {
                    skinid: default_skin_id,
                    skin_name: default_skin_name,
                    name: name_col.to_string(),
                });
            } else {
                current_char_id = None;
                current_char_name = None;
            }
        }

        // Only emit an entry if we have a valid skin ID
        let skin_id = skin_id_col.trim();
        if skin_id.is_empty() || !skin_id.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }

        let Some(ref char_id) = current_char_id else {
            continue;
        };
        let char_name = current_char_name.as_deref().unwrap_or("Unknown");
        let skin_name = if skin_name_col.is_empty() {
            // Fall back to character name for the base/default skin
            char_name.to_string()
        } else {
            skin_name_col.to_string()
        };

        entries.push(SkinEntry {
            skinid: skin_id.to_string(),
            skin_name,
            name: char_name.to_string(),
        });

        // Suppress unused variable warning when char_id is only used in the guard
        let _ = char_id;
    }

    entries
}
#[instrument(name = "fetch_mesh_list_in_bg")]
pub fn fetch_mesh_list_in_bg(
) -> thread::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>> {
    thread::spawn(|| {
        let client = reqwest::blocking::Client::new();

        let response = client
            .get("https://rivals.natimerry.com/meshes")
            .send()?
            .error_for_status()?;

        let body = response.text()?;
        let json: Vec<String> = serde_json::from_str(&body)?;

        let file = File::create("mesh_dir_list.txt")?;
        let mut writer = BufWriter::new(file);

        for line in json {
            writeln!(writer, "{}", line)?;
        }

        Ok(())
    })
}

#[derive(serde::Deserialize)]
struct CliState {
    game_path: PathBuf,
    game_chunk_path: Option<PathBuf>,
    kawaii_physics_usmap: Option<PathBuf>,
}

fn cli_config_paths() -> [PathBuf; 2] {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("repak_manager");
    [dir.join("repak_mod_manager.json"), dir.join("state.json")]
}

fn installed_iostore_paks(mods_dir: &std::path::Path) -> std::io::Result<Vec<PathBuf>> {
    let mut paks = Vec::new();
    for entry in fs::read_dir(mods_dir)? {
        let path = entry?.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("utoc") {
            continue;
        }
        let pak = path.with_extension("pak");
        let ucas = path.with_extension("ucas");
        if pak.exists() && ucas.exists() {
            paks.push(pak);
        }
    }
    paks.sort();
    Ok(paks)
}

fn run_fix_kawaii_physics_cli() -> Result<(), String> {
    let config_path = cli_config_paths()
        .into_iter()
        .find(|path| path.exists())
        .ok_or_else(|| "Could not find saved repak-rivals state".to_string())?;
    let state = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read {}: {e}", config_path.display()))?;
    let state: CliState = serde_json::from_str(&state)
        .map_err(|e| format!("Failed to parse {}: {e}", config_path.display()))?;

    let mods_dir = state.game_path;
    let game_paks_dir = state
        .game_chunk_path
        .ok_or_else(|| "No game Paks directory found in saved state".to_string())?;
    let kawaii_physics_usmap = state
        .kawaii_physics_usmap
        .ok_or_else(|| "No KawaiiPhysics USMAP file found in saved state".to_string())?;

    let paks = installed_iostore_paks(&mods_dir)
        .map_err(|e| format!("Failed to scan {}: {e}", mods_dir.display()))?;
    if paks.is_empty() {
        return Err(format!(
            "No installed IoStore mods found in {}",
            mods_dir.display()
        ));
    }

    println!("Found {} installed IoStore mods", paks.len());
    println!("Extracting installed IoStore mods to legacy assets...");
    let extracted_temp =
        tempfile::tempdir().map_err(|e| format!("Failed to create temp directory: {e}"))?;
    let extracted_dirs =
        to_legacy_uasset_fast_batch(&paks, extracted_temp.path().to_path_buf(), game_paks_dir)
            .map_err(|e| format!("Batch to-legacy extraction failed: {e}"))?;
    println!(
        "Extracted {} mods. Rebuilding fixed IoStore mods...",
        extracted_dirs.len()
    );

    let fixed_mods_dir = PathBuf::from("./fixed-mods");
    fs::create_dir_all(&fixed_mods_dir)
        .map_err(|e| format!("Failed to create {}: {e}", fixed_mods_dir.display()))?;

    for (idx, extracted_dir) in extracted_dirs.into_iter().enumerate() {
        let mod_name = extracted_dir
            .file_stem()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                format!(
                    "Invalid extracted mod directory: {}",
                    extracted_dir.display()
                )
            })?
            .to_string();
        println!("[{}/{}] Rebuilding {}", idx + 1, paks.len(), mod_name);
        let mod_output_dir = fixed_mods_dir.join(&mod_name);
        fs::create_dir_all(&mod_output_dir).map_err(|e| {
            format!(
                "Failed to create fixed mod directory {}: {e}",
                mod_output_dir.display()
            )
        })?;

        let mut mods = map_to_mods_internal(&[extracted_dir]);
        for installable_mod in &mut mods {
            installable_mod.enabled = true;
            installable_mod.is_dir = true;
            installable_mod.iostore = false;
            installable_mod.repak = false;
            installable_mod.kawaii_porter = true;
        }

        install_mods_in_viewport(
            &mut mods,
            &mod_output_dir,
            Arc::new(AtomicI32::new(0)),
            &AtomicBool::new(false),
            &None,
            &Some(kawaii_physics_usmap.clone()),
        );
    }

    println!("Wrote fixed mods to {}", fixed_mods_dir.display());
    Ok(())
}

fn main() {
    let args = args().collect::<Vec<String>>();
    #[cfg(all(windows, not(debug_assertions)))]
    let has_cli_args = args.len() > 1;
    #[cfg(not(debug_assertions))]
    let fix_kawaii_physics_cli = args
        .get(1)
        .map(|arg| arg == "--fix-kawaii-physics")
        .unwrap_or(false);

    #[cfg(target_os = "windows")]
    if !is_console() {
        detach_startup_console();
    }
    #[cfg(all(windows, not(debug_assertions)))]
    if has_cli_args {
        ensure_console();
    }
    #[cfg(target_os = "windows")]
    #[cfg(not(debug_assertions))]
    std::panic::set_hook(Box::new(move |info| {
        custom_panic(info.into());
    }));

    let exe_path = std::env::current_exe().expect("Failed to get executable path");
    let log_path = exe_path
        .parent()
        .expect("Failed to get executable directory")
        .join("latest.log");
    let _log_guard = init_tracing(&log_path);

    info!(
        "Logger initialized at {:?}; egui-family targets restricted to info and above",
        log_path
    );

    /*
        Custom baked CLI utility for tobi, if the program detects a specific argument passed to it, it does not spaw GUI
    */

    let path_reset = args.iter().any(|arg| arg == "--path-reset");
    if args.len() > 1 {
        if args[1] == "--fix-kawaii-physics" {
            if let Err(e) = run_fix_kawaii_physics_cli() {
                eprintln!("{e}");
                exit(1);
            }
            exit(0);
        }

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
        if args[1] == "--extract-dir" {
            let search_dir = &args[2];

            let mut success_log: HashMap<PathBuf, PathBuf> = HashMap::new();
            let mut failure_log: Vec<(PathBuf, String)> = Vec::new();

            println!("Searching for .utoc files in: {}", search_dir);

            for entry in WalkDir::new(search_dir)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("utoc"))
            {
                let path = entry.path().to_path_buf();
                let file_stem = match path.file_stem().and_then(|s| s.to_str()) {
                    Some(v) => v,
                    None => {
                        failure_log.push((path.clone(), "Invalid file stem".into()));
                        continue;
                    }
                };

                let parent_dir = match path.parent() {
                    Some(p) => p,
                    None => {
                        failure_log.push((path.clone(), "No parent directory".into()));
                        continue;
                    }
                };

                let result_path = parent_dir.join(format!("unpacked_{}", file_stem));

                println!("\nProcessing: {:?}", path);
                println!("Using directory: {:?}", result_path);

                // Ensure directory exists (do not fail if it already does)
                if let Err(e) = std::fs::create_dir_all(&result_path) {
                    failure_log.push((path.clone(), format!("Directory create failed: {}", e)));
                    continue;
                }

                let action = ActionUnpack {
                    utoc: path.clone(),
                    output: result_path.clone(),
                    verbose: true,
                };

                let mut config = retoc::Config {
                    container_header_version_override: None,
                    ..Default::default()
                };

                let aes_toc = match retoc::AesKey::from_str(
                    "0C263D8C22DCB085894899C3A3796383E9BF9DE0CBFB08C9BF2DEF2E84F29D74",
                ) {
                    Ok(k) => k,
                    Err(e) => {
                        failure_log.push((path.clone(), format!("Invalid AES key: {}", e)));
                        continue;
                    }
                };

                config.aes_keys.insert(FGuid::default(), aes_toc);
                let config = Arc::new(config);

                match action_unpack(action, config) {
                    Ok(_) => {
                        success_log.insert(path.clone(), result_path.clone());
                        println!("Extracted successfully");
                    }
                    Err(e) => {
                        failure_log.push((path.clone(), format!("Extraction failed: {}", e)));
                    }
                }
            }

            println!("\n{}", "=".repeat(64));
            println!("EXTRACTION SUMMARY");
            println!("{}", "=".repeat(64));

            println!("Successful extractions: {}", success_log.len());
            for (src, dst) in &success_log {
                println!("✓ {:?} → {:?}", src, dst);
            }

            println!("\nFailed extractions: {}", failure_log.len());
            for (path, reason) in &failure_log {
                println!("✗ {:?} — {}", path, reason);
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
                let count = Arc::new(AtomicI32::new(0));
                convert_directory_to_iostore(
                    &installable,
                    mod_dir.to_path_buf(),
                    paths[i].clone(),
                    count,
                    None,
                    false,
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

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1366.0, 768.0])
            .with_min_inner_size([1100.0, 650.])
            .with_drag_and_drop(true)
            .with_icon(ICON.clone()),
        ..Default::default()
    };

    // spawn a background thread to get skins
    fetch_skins_in_background();

    #[cfg(not(debug_assertions))]
    fetch_mesh_list_in_bg();
    eframe::run_native(
        "Repak GUI",
        options,
        Box::new(|cc| {
            cc.egui_ctx
                .style_mut(|style| style.visuals.dark_mode = true);
            Ok(Box::new(
                RepakModManager::load(cc, path_reset).expect("Unable to load config"),
            ))
        }),
    )
    .expect("Unable to spawn windows");
}

fn init_tracing(log_path: &std::path::Path) -> tracing_appender::non_blocking::WorkerGuard {
    let log_directory = log_path.parent().expect("Failed to get log directory");
    let log_filename = log_path
        .file_name()
        .and_then(|name| name.to_str())
        .expect("Failed to get log filename");

    let file_appender = tracing_appender::rolling::never(log_directory, log_filename);
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);
    let file_writer = install_terminal::StripAnsiMakeWriter::new(file_writer);
    let terminal_buffer_writer = install_terminal::terminal_make_writer();

    let app_filter = Targets::default()
        .with_default(LevelFilter::DEBUG)
        .with_target("egui", LevelFilter::OFF)
        .with_target("eframe", LevelFilter::OFF)
        .with_target("epaint", LevelFilter::OFF)
        .with_target("winit", LevelFilter::INFO);
    let egui_filter = Targets::default()
        .with_default(LevelFilter::OFF)
        .with_target("egui", LevelFilter::INFO)
        .with_target("eframe", LevelFilter::INFO)
        .with_target("epaint", LevelFilter::INFO);

    let terminal_format = fmt::format()
        .with_target(false)
        .with_thread_names(false)
        .with_thread_ids(false)
        .with_file(true)
        .with_line_number(true);
    let file_format = fmt::format()
        .with_ansi(false)
        .with_target(false)
        .with_thread_names(false)
        .with_thread_ids(false)
        .with_file(true)
        .with_line_number(true);

    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .event_format(terminal_format.clone())
                .with_writer(std::io::stderr)
                .with_ansi(true)
                .with_filter(app_filter.clone()),
        )
        .with(
            fmt::layer()
                .event_format(terminal_format)
                .with_writer(std::io::stderr)
                .with_ansi(true)
                .with_filter(egui_filter.clone()),
        )
        .with(
            fmt::layer()
                .event_format(fmt::format().with_target(false).with_ansi(true))
                .with_writer(terminal_buffer_writer)
                .with_ansi(true)
                .with_filter(app_filter.clone()),
        )
        .with(
            fmt::layer()
                .event_format(file_format.clone())
                .with_writer(file_writer.clone())
                .with_ansi(false)
                .with_filter(app_filter),
        )
        .with(
            fmt::layer()
                .event_format(file_format)
                .with_writer(file_writer)
                .with_ansi(false)
                .with_filter(egui_filter),
        )
        .init();

    guard
}
