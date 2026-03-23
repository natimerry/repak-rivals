use crate::utoc_utils::read_utoc;
use eframe::egui;
use eframe::egui::OutputCommand::CopyText;
use eframe::egui::RichText;
use egui_extras::{Column, TableBuilder};
use repak::PakReader;
use rfd::FileDialog;
use sha2::Digest;
use std::fs::File;
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;

pub struct FileTable {
    striped: bool,
    resizable: bool,
    clickable: bool,
    // scroll_to_row_slider: usize,
    // scroll_to_row: Option<usize>,
    file_contents: Vec<FileEntry>,
    selection: usize,
    showing_utoc: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct FileEntry {
    /// This is the path of the internal file mounting path
    pub(crate) file_path: String,
    pub(crate) pak_path: PathBuf,
    // entry: Entry,
    pub(crate) pak_reader: PakReader,
    pub(crate) compressed: String,
    pub(crate) uncompressed: String,
    pub(crate) offset: String,
    pub(crate) bulkdata: Option<usize>,
    pub(crate) package_data: Option<usize>,
}
impl Default for FileTable {
    fn default() -> Self {
        Self {
            striped: true,
            resizable: true,
            clickable: true,
            file_contents: vec![],
            selection: usize::MAX,
            showing_utoc: false,
        }
    }
}

impl FileTable {
    pub fn new(pak_reader: &PakReader, pak_path: &Path) -> Self {
        // If the utoc exists, use the utoc

        let mut utoc_path = pak_path.to_path_buf();
        utoc_path.set_extension("utoc");

        let file_entries = {
            if utoc_path.exists() {
                read_utoc(&utoc_path, pak_reader, pak_path)
            } else {
                let entries = pak_reader.files().to_vec();

                entries
                    .iter()
                    .map(|entry| {
                        let entry_pak = pak_reader.get_file_entry(entry).unwrap();
                        FileEntry {
                            file_path: entry.clone(),
                            pak_path: PathBuf::from(pak_path),
                            pak_reader: pak_reader.clone(),
                            // entry: pak_reader.get_file_entry(entry).unwrap(),
                            compressed: entry_pak.compressed.to_string(),
                            uncompressed: entry_pak.uncompressed.to_string(),
                            offset: format!("{:#x}", entry_pak.offset),
                            bulkdata: None,
                            package_data: None,
                        }
                    })
                    .collect::<Vec<_>>()
            }
        };

        Self {
            file_contents: file_entries,
            showing_utoc: utoc_path.exists(),
            ..Default::default()
        }
    }

    pub fn table_ui(&mut self, ui: &mut egui::Ui) {
        let available_height = ui.available_height();
        let mut table = TableBuilder::new(ui)
            .striped(self.striped)
            .resizable(self.resizable)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(
                Column::initial(800.)
                    .at_least(500.0)
                    .clip(true)
                    .resizable(true)
                    .at_most(1000.),
            ) // PATH
            .column(Column::remainder()) // Offset
            .column(Column::remainder()) // Compressed Size
            .column(Column::remainder()) // Uncompressed Size
            .min_scrolled_height(0.0)
            .max_scroll_height(available_height);

        if self.clickable {
            table = table.sense(egui::Sense::click());
        }

        table
            .header(20.0, |mut header| {
                header.col(|ui| {
                    ui.label("Path");
                });

                if self.showing_utoc {
                    header.col(|ui| {
                        ui.label("Bulkdata Chunks");
                    });

                    header.col(|ui| {
                        ui.label("PackageData Chunks");
                    });
                } else {
                    header.col(|ui| {
                        ui.label("Offset");
                    });

                    header.col(|ui| {
                        ui.label("Compressed");
                    });
                    header.col(|ui| {
                        ui.label("Uncompressed");
                    });
                }
                // header.col(|ui| {
                //     ui.label("Compression Slot");
                // });
            })
            .body(|body| {
                // let mut file = self.file_contents.clone();
                body.rows(20.0, self.file_contents.len(), |mut row| {
                    let row_idx = row.index();

                    let entry = &mut self.file_contents[row_idx];
                    row.set_selected(self.selection == row_idx);
                    row.col(|ui| {
                        ui.visuals_mut().widgets.hovered = ui.visuals().widgets.inactive;
                        if ui.label(RichText::new(&entry.file_path).strong()).clicked() {
                            self.selection = row_idx;
                        };
                    })
                    .1
                    .context_menu(|ui| show_ctx_menu(ui, entry, self.showing_utoc));

                    if self.showing_utoc {
                        row.col(|ui| {
                            ui.label(&entry.bulkdata.unwrap_or(0).to_string());
                        });
                        row.col(|ui| {
                            ui.label(&entry.package_data.unwrap_or(0).to_string());
                        });
                    } else {
                        row.col(|ui| {
                            ui.label(&entry.offset);
                        });
                        row.col(|ui| {
                            ui.label(&entry.compressed);
                        });
                        row.col(|ui| {
                            ui.label(&entry.uncompressed);
                        });
                    }

                    self.toggle_row_selection(row_idx, &row.response());
                });
            });
    }
    fn toggle_row_selection(&mut self, row_index: usize, row_response: &egui::Response) {
        if row_response.clicked() {
            self.selection = row_index;
        }
    }
}
fn show_ctx_menu(ui: &mut egui::Ui, entry: &FileEntry, is_utoc: bool) {
    if ui.button("Extract").clicked() {
        if is_utoc {
            let name = PathBuf::from(&entry.file_path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string();
            let dialog = FileDialog::new().set_file_name(name).save_file();

            if let Some(path) = dialog {
                let root = path.file_stem().unwrap().to_str().unwrap();
                let result_path = path.parent().unwrap().join(root);
                println!("Creating directory: {:?}", &result_path);

                let _ = std::fs::create_dir(&result_path)
                    .expect("Failed to create extraction directory");

                let action: retoc::ActionUnpack = retoc::ActionUnpack {
                    utoc: PathBuf::from(&entry.file_path),
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

                config
                    .aes_keys
                    .insert(retoc::FGuid::default(), aes_toc.clone());
                let config = std::sync::Arc::new(config);

                retoc::action_unpack(action, config).expect("Failed to extract");
            }
            ui.label("This is a utoc file, you can't extract files from it.");
            ui.close_menu();
        }

        let name = PathBuf::from(&entry.file_path)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let dialog = FileDialog::new().set_file_name(name).save_file();
        if let Some(path) = dialog {
            let pak_reader = &entry.pak_reader;
            let mut reader =
                BufReader::new(File::open(&entry.pak_path).expect("Failed to open pak file"));

            let buffer = pak_reader
                .get(entry.file_path.as_str(), &mut reader)
                .expect("Failed to read file");

            let mut file = File::create(path).expect("Failed to create file");
            file.write_all(&buffer).expect("Failed to write file");
            ui.close_menu();
        }
    }

    if ui.button("Copy Path").clicked() {
        ui.output_mut(|o| o.commands = vec![CopyText(entry.file_path.clone())]);
        ui.close_menu();
    }
    if !is_utoc {
        if ui.button("Copy Offset").clicked() {
            ui.output_mut(|o| o.commands = vec![CopyText(entry.offset.clone().to_string())]);
            ui.close_menu();
        }
    }

    let mut hasher = sha2::Sha256::new();
    entry
        .pak_reader
        .read_file(
            entry.file_path.as_str(),
            &mut BufReader::new(File::open(&entry.pak_path).expect("Failed to open pak file")),
            &mut hasher,
        )
        .expect("Failed to read file");

    if ui
        .button("View Hash (Click to copy)")
        .on_hover_text(RichText::new(format!(
            "SHA256 hash: {}",
            hex::encode(hasher.clone().finalize())
        )))
        .clicked()
    {
        ui.output_mut(|o| o.commands = vec![CopyText(hex::encode(hasher.finalize()))]);
    }
}
