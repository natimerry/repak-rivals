use crate::install_mod::ModConflicts::{Hard, Soft};
use crate::pak_logic::install_mods_in_viewport;
use crate::utils::{collect_files, get_character_mod_skin, get_current_pak_characteristics};
use crate::{setup_custom_style, ICON};
use eframe::egui;
use eframe::egui::{Align, Checkbox, ComboBox, Context, Label, TextEdit};
use egui_extras::{Column, TableBuilder};
use egui_flex::{item, Flex, FlexAlign};
use log::{debug, error, info, warn};
use rayon::iter::IntoParallelRefIterator;
use regex_lite::Regex;
use repak::utils::AesKey;
use repak::{Compression, PakReader};
use sha2::digest::block_buffer::Lazy;
use simd_str_cmp::{
    compare_string_vectors, compare_string_vectors_simd, compare_string_vectors_simd_non_par,
};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::atomic::{AtomicBool, AtomicI32};
use std::sync::{Arc, LazyLock};
use std::thread;
use std::thread::sleep;
use std::time::Duration;

#[derive(Debug, Default, Clone)]
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
}
impl ModInstallRequest {
    pub fn new(mods: Vec<InstallableMod>, mod_directory: PathBuf) -> Self {
        let len = mods.len();
        check_mod_file_collisions(&mods);
        Self {
            animate: false,
            mods,
            mod_directory,
            total_mods: len as f32,
            installed_mods_cbk: Arc::new(AtomicI32::new(0)),
            joined_thread: None,
            stop_thread: Arc::new(AtomicBool::new(false)),
        }
    }
}

enum ModConflicts {
    Hard(String),
    Soft(String),
}
impl ModInstallRequest {
    pub fn new_mod_dialog(&mut self, ctx: &egui::Context, show_callback: &mut bool) {
        let viewport_options = egui::ViewportBuilder::default()
            .with_title("Install mods")
            .with_icon(ICON.clone())
            .with_inner_size([1000.0, 800.0])
            .with_always_on_top();

        Context::show_viewport_immediate(
            &ctx,
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
                                    self.stop_thread.store(true, SeqCst);
                                    *show_callback = false;
                                });

                                if install_mod.clicked() {
                                    let mut mods =
                                        self.mods.iter().map(|x| x.clone()).collect::<Vec<_>>(); // clone

                                    let collisions = check_mod_file_collisions(&mods);
                                    if collisions.is_empty() {
                                        let dir = self.mod_directory.clone();
                                        let new_atomic = self.installed_mods_cbk.clone();
                                        let new_stop_thread = self.stop_thread.clone();
                                        self.joined_thread = Some(std::thread::spawn(move || {
                                            install_mods_in_viewport(
                                                &mut mods,
                                                &dir,
                                                &new_atomic,
                                                &new_stop_thread,
                                            );
                                        }));
                                        self.animate = true;
                                    }
                                    let hard_conflicts = collisions
                                        .iter()
                                        .filter(|x| match x {
                                            Soft(_) => false,
                                            Hard(_) => true,
                                        })
                                        .collect::<Vec<_>>();

                                    if !hard_conflicts.is_empty() {
                                        rfd::MessageDialog::new()
                                            .set_title("Hard conflicts detected!")
                                            .set_description(format!(
                                                "{} conflicts detected",
                                                hard_conflicts.len()
                                            )).show();
                                        return;
                                    }
                                }
                            });

                        let total_mods = self.total_mods.clone() as f32;
                        let installed = self
                            .installed_mods_cbk
                            .load(std::sync::atomic::Ordering::SeqCst);
                        let mut percentage = installed as f32 / total_mods;
                        if installed == -255 {
                            percentage = 1.0;
                        }
                        ui.add(
                            egui::ProgressBar::new(percentage)
                                .text("Installing mods...")
                                .animate(self.animate)
                                .show_percentage(),
                        );

                        if installed == -255 {
                            self.animate = false;
                            *show_callback = false;
                        }
                    });
                if ctx.input(|i| i.viewport().close_requested()) {
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
            .column(Column::remainder().at_least(400.)) // mod name
            .column(Column::auto()) // Character
            .column(Column::auto()) // type
            .column(Column::remainder()) // options
            .min_scrolled_height(0.0)
            .max_scroll_height(available_height);

        table
            .header(20., |mut header| {
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
                            let label = match mods.is_dir {
                                true => "Directory",
                                false => "Pak",
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
                                    mods.is_dir || mods.repak,
                                    Checkbox::new(&mut mods.fix_mesh, "Fix mesh"),
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

pub const AES_KEY: LazyLock<AesKey> = LazyLock::new(|| {
    AesKey::from_str("0C263D8C22DCB085894899C3A3796383E9BF9DE0CBFB08C9BF2DEF2E84F29D74")
        .expect("Unable to initialise AES_KEY")
});

fn build_conflict_path_list(installable_mod: &InstallableMod) -> Vec<String> {
    use regex_lite::Regex;
    let mut files = vec![];
    if installable_mod.is_dir {
        let mut paths = Vec::new();
        collect_files(&mut paths, &installable_mod.mod_path).expect("Failed to collect files");
        files = paths
            .iter()
            .map(|x| x.to_str().unwrap().to_string())
            .collect::<Vec<String>>();
    } else {
        files = installable_mod.reader.clone().unwrap().files();
    }

    files
}

fn check_mod_file_collisions(mod_list: &[InstallableMod]) -> Vec<ModConflicts> {
    let mut conflicts: Vec<ModConflicts> = Vec::new();
    for (i, mod1) in mod_list.iter().enumerate() {
        let mut files1: Vec<String> = build_conflict_path_list(mod1);
        let mod1_type = mod1.mod_type.clone();
        debug!("Len of files21 {}", files1.len());
        for mod2 in mod_list.iter().skip(i + 1) {
            let mod2_type = mod2.mod_type.clone();

            if mod1_type == mod2_type {
                conflicts.push(Soft(mod1_type.clone()))
            }
            let mut files2: Vec<String> = build_conflict_path_list(mod2);
            debug!("Len of files2: {}", files2.len());
            let conflict_idx;
            if files1.len() * files2.len() < 2500 {
                conflict_idx = compare_string_vectors_simd(&files1, &files2)
            } else {
                conflict_idx = compare_string_vectors_simd_non_par(&files2, &files1)
            }

            if conflict_idx.is_empty() {
                debug!("Mod1 and Mod2 have no hard conflicts");
            } else {
                println!("Mod1 and Mod2 have {} conflicts", conflict_idx.len());
                for (i, j) in conflict_idx {
                    conflicts.push(Hard(files1[i].clone()));
                    warn!(
                        "Conflicting {} in {} and {}",
                        files1[i], &mod1.mod_name, &mod2.mod_name
                    );
                }
            }
        }
    }
    conflicts
}

pub fn map_paths_to_mods(paths: &Vec<PathBuf>) -> Vec<InstallableMod> {
    let installable_mods = paths
        .into_iter()
        .map(|path| {
            let is_dir = path.clone().is_dir();

            let mut modtype = "Unknown".to_string();
            let mut pak = None;

            if !is_dir {
                let builder = repak::PakBuilder::new()
                    .key(AES_KEY.clone().0)
                    .reader(&mut BufReader::new(File::open(path.clone()).unwrap()));

                match builder {
                    Ok(builder) => {
                        pak = Some(builder.clone());
                        modtype = get_current_pak_characteristics(builder.files());
                    }
                    Err(e) => {
                        error!("Error reading pak file: {}", e);
                        return Err(e);
                    }
                }
            }
            if let None = pak {
                assert!(is_dir);
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
                ..Default::default()
            })
        })
        .filter_map(|x: Result<InstallableMod, repak::Error>| x.ok())
        .collect::<Vec<_>>();
    installable_mods
}

pub fn map_dropped_file_to_mods(dropped_files: &Vec<egui::DroppedFile>) -> Vec<InstallableMod> {
    let files = dropped_files
        .into_iter()
        .map(|dropped_file| {
            let is_dir = dropped_file.path.clone().unwrap().is_dir();
            let mut modtype = "Unknown".to_string();

            let mut pak = None;

            let pakfile = dropped_file.path.clone().unwrap();
            if !is_dir {
                let builder = repak::PakBuilder::new()
                    .key(AES_KEY.clone().0)
                    .reader(&mut BufReader::new(File::open(pakfile.clone()).unwrap()));
                match builder {
                    Ok(builder) => {
                        pak = Some(builder.clone());
                        modtype = get_current_pak_characteristics(builder.files());
                    }
                    Err(e) => {
                        error!("Error reading pak file: {}", e);
                        return Err(e);
                    }
                }
            }
            if let None = pak {
                assert!(is_dir);
            }

            Ok(InstallableMod {
                mod_name: pakfile.file_stem().unwrap().to_str().unwrap().to_string(),
                mod_type: modtype,
                repak: !is_dir,
                fix_mesh: false,
                is_dir,
                reader: pak,
                mod_path: pakfile.clone(),
                mount_point: "../../../".to_string(),
                path_hash_seed: "00000000".to_string(),
                ..Default::default()
            })
        })
        .filter_map(|x: Result<InstallableMod, repak::Error>| x.ok())
        .collect::<Vec<_>>();
    files
}
