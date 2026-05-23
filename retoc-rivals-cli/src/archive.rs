use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use unrar::Archive;
use zip::ZipArchive;

pub fn extract_archive(path: &Path) -> Result<TempDir, String> {
    let temp = tempfile::tempdir().map_err(|e| format!("Failed to create temp dir: {e}"))?;
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("zip") => extract_zip(path, temp.path())
            .map_err(|e| format!("Failed to extract {}: {e}", path.display()))?,
        Some("rar") => extract_rar(path, temp.path())
            .map_err(|e| format!("Failed to extract {}: {e}", path.display()))?,
        Some("7z") => sevenz_rust2::decompress_file(path, temp.path())
            .map_err(|e| format!("Failed to extract {}: {e}", path.display()))?,
        _ => return Err(format!("Unsupported archive: {}", path.display())),
    }
    Ok(temp)
}

fn extract_zip(zip_path: &Path, output_dir: &Path) -> io::Result<()> {
    let file = File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = output_dir.join(file.mangled_name());

        if file.name().ends_with('/') {
            std::fs::create_dir_all(&outpath)?;
        } else {
            if let Some(parent) = outpath.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut outfile = File::create(&outpath)?;
            io::copy(&mut file, &mut outfile)?;
        }
    }

    Ok(())
}

fn extract_rar(rar_path: &Path, output_dir: &Path) -> Result<(), unrar::error::UnrarError> {
    let mut archive = Archive::new(rar_path).open_for_processing()?;
    while let Some(header) = archive.read_header()? {
        let filename: PathBuf = header.entry().filename.clone();
        archive = if header.entry().is_file() {
            let output = output_dir.join(filename);
            if let Some(parent) = output.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            header.extract_to(output)?
        } else {
            header.skip()?
        };
    }
    Ok(())
}
