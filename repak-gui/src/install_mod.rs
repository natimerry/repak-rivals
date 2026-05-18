pub mod install_mod_logic;

use crate::install_mod::install_mod_logic::archives::*;
use crate::main_ui::setup_custom_style;
use crate::utils::{collect_files, get_current_pak_characteristics};
use crate::utoc_utils::read_utoc;
use crate::ICON;
use eframe::egui;
use eframe::egui::{Align, Checkbox, ComboBox, Context, Label, TextEdit};
use egui_extras::{Column, TableBuilder};
use egui_flex::{item, Flex, FlexAlign};
use install_mod_logic::install_mods_in_viewport;
use repak::utils::AesKey;
use repak::Compression::Oodle;
use repak::{Compression, PakReader};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
#[cfg(all(windows, not(debug_assertions)))]
use std::process::Command;
use std::str::FromStr;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::atomic::{AtomicBool, AtomicI32};
use std::sync::{Arc, LazyLock};
use std::thread;
use tempfile::tempdir;
use tracing::{debug, error, info, instrument, warn};
use walkdir::WalkDir;

#[cfg(all(windows, not(debug_assertions)))]
fn spawn_install_terminal() {
    match Command::new("cmd")
        .args([
            "/C",
            "start",
            "repak-rivals install",
            "cmd",
            "/K",
            "echo repak-rivals install started. Close this window when finished.",
        ])
        .spawn()
    {
        Ok(_) => info!("Spawned install terminal"),
        Err(err) => warn!(error = %err, "Failed to spawn install terminal"),
    }
}

#[cfg(any(not(windows), debug_assertions))]
fn spawn_install_terminal() {}

#[derive(Debug, Clone)]
pub struct InstallableMod {
    pub mod_name: String,
    pub mod_type: String,
    pub repak: bool,
    pub fix_mesh: bool,
    pub is_dir: bool,
    pub editing: bool,
    pub path_hash_seed: String,
    pub mount_point: String,
    pub compression: Compression,
    pub reader: Option<PakReader>,
    pub mod_path: PathBuf,
    pub total_files: usize,
    pub iostore: bool,
    // the only reason we keep this is to filter out the archives during collection
    pub is_archived: bool,
    pub enabled: bool,
    pub obfuscated: bool,
    // pub audio_mod: bool,
}

impl Default for InstallableMod {
    fn default() -> Self {
        InstallableMod {
            obfuscated: false,
            mod_name: "".to_string(),
            mod_type: "".to_string(),
            repak: false,
            fix_mesh: false,
            is_dir: false,
            editing: false,
            path_hash_seed: "".to_string(),
            mount_point: "".to_string(),
            compression: Default::default(),
            reader: None,
            mod_path: Default::default(),
            total_files: 0,
            iostore: false,
            is_archived: false,
            enabled: true,
        }
    }
}

fn processable_asset_count<'a>(paths: impl IntoIterator<Item = &'a str>) -> usize {
    paths
        .into_iter()
        .filter(|path| path.to_ascii_lowercase().ends_with(".uasset"))
        .count()
        .max(1)
}

#[derive(Debug)]
pub struct ModInstallRequest {
    pub(crate) mods: Vec<InstallableMod>,
    pub mod_directory: PathBuf,
    pub animate: bool,
    pub total_mods: f32,
    pub installed_mods_cbk: Arc<AtomicI32>,
    pub joined_thread: Option<thread::JoinHandle<()>>,
    pub stop_thread: Arc<AtomicBool>,
    pub chunkdir: Option<PathBuf>,
}
impl ModInstallRequest {
    #[instrument(skip(mods), fields(mod_count = mods.len(), mod_directory = ?mod_directory))]
    pub fn new(
        mods: Vec<InstallableMod>,
        mod_directory: PathBuf,
        chunkdir: &Option<PathBuf>,
    ) -> Self {
        let len = mods
            .iter()
            .filter(|m| m.enabled)
            .map(|m| m.total_files)
            .sum::<usize>()
            .max(1);
        info!("Created install request");
        let chunkdir = chunkdir.clone();
        Self {
            animate: false,
            mods,
            mod_directory,
            total_mods: len as f32,
            installed_mods_cbk: Arc::new(AtomicI32::new(0)),
            joined_thread: None,
            stop_thread: Arc::new(AtomicBool::new(false)),
            chunkdir,
        }
    }
}

impl ModInstallRequest {
    #[instrument(skip(self, ctx, show_callback), fields(mod_count = self.mods.len(), mod_directory = ?self.mod_directory))]
    pub fn new_mod_dialog(&mut self, ctx: &egui::Context, show_callback: &mut bool) {
        let viewport_options = egui::ViewportBuilder::default()
            .with_title("Install mods")
            .with_icon(ICON.clone())
            .with_inner_size([1000.0, 800.0])
            .with_always_on_top();

        Context::show_viewport_immediate(
            ctx,
            egui::ViewportId::from_hash_of("immediate_viewport"),
            viewport_options,
            |ctx, class| {
                assert!(
                    class == egui::ViewportClass::Immediate,
                    "This egui backend doesn't support multiple viewports"
                );

                setup_custom_style(ctx);
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.label("Mods to install");
                    ui.set_min_width(ui.available_width());
                    ui.set_min_height(ui.available_height());
                    // ScrollArea::vertical()
                    //     .auto_shrink([false, false])
                    //     .show(ui, |ui| {
                    self.table_ui(ui);
                    // });
                });
                egui::TopBottomPanel::bottom("bottom_panel")
                    .min_height(50.)
                    .show(ctx, |ui| {
                        // ui.heading("WTF");
                        ui.set_min_width(ui.available_width());
                        ui.set_min_height(ui.available_height());
                        Flex::horizontal()
                            .align_items(FlexAlign::Center)
                            .w_auto()
                            .h_auto()
                            .show(ui, |ui| {
                                let selection_bg_color = ctx.style().visuals.selection.bg_fill;

                                let install_mod = ui.add(
                                    item(),
                                    egui::Button::new("Install mod").fill(selection_bg_color),
                                );

                                let cancel = ui.add(item(), egui::Button::new("Cancel"));
                                cancel.clicked().then(|| {
                                    info!("Cancelling install dialog");
                                    self.stop_thread.store(true, SeqCst);
                                    *show_callback = false;
                                });

                                if install_mod.clicked() {
                                    info!("Starting install worker");
                                    spawn_install_terminal();
                                    let mut mods = self.mods.to_vec(); // clone
                                    self.total_mods = mods
                                        .iter()
                                        .filter(|m| m.enabled)
                                        .map(|m| m.total_files)
                                        .sum::<usize>()
                                        .max(1) as f32;
                                    self.installed_mods_cbk.store(0, SeqCst);

                                    let dir = self.mod_directory.clone();
                                    let new_atomic = self.installed_mods_cbk.clone();
                                    let new_stop_thread = self.stop_thread.clone();
                                    let chunkdir = self.chunkdir.clone();
                                    self.joined_thread = Some(std::thread::spawn(move || {
                                        install_mods_in_viewport(
                                            &mut mods,
                                            &dir,
                                            new_atomic,
                                            &new_stop_thread,
                                            &chunkdir,
                                        );
                                    }));
                                    self.animate = true;
                                }
                            });

                        let total_mods = self.total_mods;
                        let installed = self
                            .installed_mods_cbk
                            .load(std::sync::atomic::Ordering::SeqCst);
                        let mut percentage = if total_mods > 0.0 {
                            installed.max(0) as f32 / total_mods
                        } else {
                            0.0
                        };
                        percentage = percentage.clamp(0.0, 1.0);
                        if installed == -255 {
                            percentage = 1.0;
                        }
                        ui.add(
                            egui::ProgressBar::new(percentage)
                                .text(format!(
                                    "Installing assets... {}/{}",
                                    installed.max(0),
                                    total_mods as i32
                                ))
                                .animate(self.animate)
                                .show_percentage(),
                        );

                        if installed == -255 {
                            self.animate = false;
                            *show_callback = false;
                        }
                    });
                if ctx.input(|i| i.viewport().close_requested()) {
                    warn!("Install dialog viewport was closed");
                    // Tell parent viewport that we should not show next frame:
                    *show_callback = false;
                }
            },
        );
    }

    fn table_ui(&mut self, ui: &mut egui::Ui) {
        let available_height = ui.available_height();
        ui.separator();

        let table = TableBuilder::new(ui)
            .striped(true)
            // .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::LEFT))
            .column(Column::auto())
            .column(Column::auto())
            .column(Column::remainder().at_least(400.)) // mod name
            .column(Column::auto()) // Character
            .column(Column::auto()) // type
            .column(Column::remainder()) // options
            .min_scrolled_height(0.0)
            .max_scroll_height(available_height);

        table
            .header(20., |mut header| {
                header.col(|ui| {
                    ui.label("Enabled");
                });

                header.col(|ui| {
                    ui.label("Row");
                });

                header.col(|ui| {
                    ui.label("Name");
                });

                header.col(|ui| {
                    ui.label("Category");
                });

                header.col(|ui| {
                    ui.label("Mod type");
                });
                header.col(|ui| {
                    ui.label("Options");
                });
            })
            .body(|mut body| {
                for (rowidx, mods) in self.mods.iter_mut().enumerate() {
                    body.row(20., |mut row| {
                        row.col(|ui| {
                            ui.add(Checkbox::new(&mut mods.enabled, ""));
                        });

                        row.col(|ui| {
                            ui.add(Label::new(format!("{})", rowidx + 1)).halign(Align::RIGHT));
                        });
                        // name field
                        row.col(|ui| {
                            ui.horizontal(|ui| {
                                if mods.editing {
                                    ui.set_width(ui.available_width() * 0.8); // padding for edit
                                    let text_edit = ui.add(
                                        egui::TextEdit::singleline(&mut mods.mod_name)
                                            .clip_text(true),
                                    );
                                    if text_edit.lost_focus()
                                        || ui.input(|i| i.key_pressed(egui::Key::Enter))
                                    {
                                        mods.editing = false;
                                    }
                                } else {
                                    ui.add(
                                        Label::new(&mods.mod_name).halign(Align::LEFT).truncate(),
                                    );
                                }
                                // align button right
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.button(if mods.editing { "✔" } else { "✏" }).clicked()
                                        {
                                            mods.editing = !mods.editing;
                                        }
                                    },
                                );
                            });
                        });
                        row.col(|ui| {
                            ui.label(&mods.mod_type);
                        });
                        row.col(|ui| {
                            let label = if mods.is_dir {
                                "Directory"
                            } else if mods.iostore {
                                "Iostore"
                            } else {
                                "Pakfile"
                            };
                            ui.label(label);
                        });
                        row.col(|ui| {
                            ui.collapsing("Options", |ui| {
                                ui.add_enabled(
                                    !mods.is_dir,
                                    Checkbox::new(&mut mods.repak, "To repak"),
                                );
                                ui.add_enabled(
                                    (mods.is_dir || mods.repak) && false, // new retoc should auto patch
                                    Checkbox::new(&mut mods.fix_mesh, "Fix mesh (outdated)"),
                                );

                                ui.add_enabled(
                                    mods.is_dir || mods.repak,
                                    Checkbox::new(&mut mods.obfuscated, "Obfuscate"),
                                );

                                let text_edit = TextEdit::singleline(&mut mods.mount_point);
                                ui.add(text_edit.hint_text("Enter mount point..."));

                                // Text edit for path_hash_seed with hint
                                let text_edit = TextEdit::singleline(&mut mods.path_hash_seed);
                                ui.add(text_edit.hint_text("Enter path hash seed..."));

                                ComboBox::new("comp_level", "Compression Algorithm")
                                    .selected_text(format!("{:?}", mods.compression))
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(
                                            &mut mods.compression,
                                            Compression::Zlib,
                                            "Zlib",
                                        );
                                        ui.selectable_value(
                                            &mut mods.compression,
                                            Compression::Gzip,
                                            "Gzip",
                                        );
                                        ui.selectable_value(
                                            &mut mods.compression,
                                            Compression::Oodle,
                                            "Oodle",
                                        );
                                        ui.selectable_value(
                                            &mut mods.compression,
                                            Compression::Zstd,
                                            "Zstd",
                                        );
                                        ui.selectable_value(
                                            &mut mods.compression,
                                            Compression::LZ4,
                                            "LZ4",
                                        );
                                    });
                            });
                        });
                    })
                }
            });
    }
}

pub static AES_KEY: LazyLock<AesKey> = LazyLock::new(|| {
    AesKey::from_str("0C263D8C22DCB085894899C3A3796383E9BF9DE0CBFB08C9BF2DEF2E84F29D74")
        .expect("Unable to initialise AES_KEY")
});

#[instrument(fields(path))]
fn find_mods_from_archive(path: &str) -> Vec<InstallableMod> {
    let mut new_mods = Vec::<InstallableMod>::new();
    debug!("Scanning extracted archive directory");
    for entry in WalkDir::new(path) {
        let entry = entry.expect("Failed to read directory entry");
        let path = entry.path();
        if path.is_file() {
            let builder = repak::PakBuilder::new()
                .key(AES_KEY.clone().0)
                .reader(&mut BufReader::new(File::open(path).unwrap()));

            if let Ok(builder) = builder {
                let mut len = 1;
                let mut modtype = String::from("Unknown");
                let mut iostore = false;

                let pak_path = path.with_extension("pak");
                let utoc_path = path.with_extension("utoc");
                let ucas_path = path.with_extension("ucas");

                if pak_path.exists() && utoc_path.exists() && ucas_path.exists() {
                    // this is a mod of type s2, create a new Installable mod from its characteristics
                    let utoc_path = path.with_extension("utoc");

                    let files = read_utoc(&utoc_path, &builder, &path);
                    let files = files
                        .iter()
                        .map(|x| x.file_path.clone())
                        .collect::<Vec<_>>();
                    len = processable_asset_count(files.iter().map(String::as_str));
                    modtype = get_current_pak_characteristics(files);
                    iostore = true;
                }
                // IF ONLY PAK IS FOUND WE NEED TO EXTRACT AND INSTALL THE PAK
                else if pak_path.exists() {
                    let files = builder.files();
                    len = processable_asset_count(files.iter().map(String::as_str));
                    modtype = get_current_pak_characteristics(files);
                }

                let installable_mod = InstallableMod {
                    mod_name: path.file_stem().unwrap().to_str().unwrap().to_string(),
                    mod_type: modtype.to_string(),
                    repak: true,
                    fix_mesh: false,
                    is_dir: false,
                    reader: Some(builder),
                    mod_path: path.to_path_buf(),
                    mount_point: "../../../".to_string(),
                    path_hash_seed: "00000000".to_string(),
                    total_files: len,
                    iostore,
                    is_archived: false,
                    editing: false,
                    compression: Oodle,
                    ..Default::default()
                };

                debug!(mod_name = %installable_mod.mod_name, mod_type = %installable_mod.mod_type, "Discovered archived mod");
                new_mods.push(installable_mod);
            }
        }
    }

    new_mods
}

#[instrument(skip(paths), fields(path_count = paths.len()))]
pub fn map_to_mods_internal(paths: &[PathBuf]) -> Vec<InstallableMod> {
    let mut extensible_vec: Vec<InstallableMod> = Vec::new();
    info!("Mapping paths into installable mods");
    let mut installable_mods = paths
        .iter()
        .map(|path| {
            let is_dir = path.clone().is_dir();
            let extension = path.extension().unwrap_or_default();
            let is_archive = extension == "zip" || extension == "rar";

            let mut modtype = "Unknown".to_string();
            let mut pak = None;
            let mut len = 1;

            if !is_dir && !is_archive {
                debug!(?path, "Inspecting pak file");
                let builder = repak::PakBuilder::new()
                    .key(AES_KEY.clone().0)
                    .reader(&mut BufReader::new(File::open(path.clone()).unwrap()));
                match builder {
                    Ok(builder) => {
                        pak = Some(builder.clone());
                        let files = builder.files();
                        len = processable_asset_count(files.iter().map(String::as_str));
                        modtype = get_current_pak_characteristics(files);
                    }
                    Err(e) => {
                        error!(?path, error = %e, "Error reading pak file");
                        return Err(e);
                    }
                }
            }

            if is_dir {
                debug!(?path, "Inspecting directory mod");
                let mut files = vec![];
                collect_files(&mut files, path)?;
                let files = files
                    .iter()
                    .map(|s| s.to_str().unwrap().to_string())
                    .collect::<Vec<_>>();
                len = processable_asset_count(files.iter().map(String::as_str));
                modtype = get_current_pak_characteristics(files);
            }

            if is_archive {
                info!(?path, "Extracting archive for inspection");
                modtype = "Season 2 Archives".to_string();
                let tempdir = tempdir()
                    .unwrap()
                    .path()
                    .as_os_str()
                    .to_str()
                    .unwrap()
                    .to_string();

                if extension == "zip" {
                    extract_zip(path.to_str().unwrap(), &tempdir).expect("Unable to install mod")
                } else if extension == "rar" {
                    extract_rar(path.to_str().unwrap(), &tempdir).expect("Unable to install mod")
                }

                // Now find pak files / s2 archives and turn them into installable mods
                let mut new_mods = find_mods_from_archive(&tempdir);
                extensible_vec.append(&mut new_mods);
            }

            Ok(InstallableMod {
                mod_name: path.file_stem().unwrap().to_str().unwrap().to_string(),
                mod_type: modtype,
                repak: !is_dir,
                fix_mesh: false,
                is_dir,
                reader: pak,
                mod_path: path.clone(),
                mount_point: "../../../".to_string(),
                path_hash_seed: "00000000".to_string(),
                total_files: len,
                is_archived: is_archive,
                ..Default::default()
            })
        })
        .filter_map(|x: Result<InstallableMod, repak::Error>| x.ok())
        .filter(|x| !x.is_archived)
        .collect::<Vec<_>>();

    installable_mods.extend(extensible_vec);

    // debug!("Mapped installable mods: {:?}", installable_mods);
    installable_mods
}

pub fn map_paths_to_mods(paths: &[PathBuf]) -> Vec<InstallableMod> {
    let installable_mods = map_to_mods_internal(paths);
    installable_mods
}

pub fn map_dropped_file_to_mods(dropped_files: &[egui::DroppedFile]) -> Vec<InstallableMod> {
    let paths = dropped_files
        .iter()
        .map(|f| f.path.clone().unwrap())
        .collect::<Vec<_>>();

    let installable_mods = map_to_mods_internal(&paths);
    installable_mods
}
