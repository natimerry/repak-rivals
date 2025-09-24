pub fn extract_zip(zip_path: &str, output_dir: &str) -> io::Result<()> {
    let file = File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = Path::new(output_dir).join(file.mangled_name());

        if file.name().ends_with('/') {
            std::fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    std::fs::create_dir_all(p)?;
                }
            }
            let mut outfile = File::create(&outpath)?;
            io::copy(&mut file, &mut outfile)?;
        }
    }

    Ok(())
}

use std::fs::File;
use std::io;
use unrar::Archive;
use std::path::Path;
use zip::ZipArchive;

pub fn extract_rar(rar_path: &str, output_dir: &str) -> Result<(), unrar::error::UnrarError> {
    let output_dir = Path::new(output_dir);
    let mut archive =
        Archive::new(rar_path)
            .open_for_processing()?;
    while let Some(header) = archive.read_header()? {
        let filename = header.entry().filename.clone();
        archive = if header.entry().is_file() {
            header.extract_to(output_dir.join(filename))?
        } else {
            header.skip()?
        };
    }
    Ok(())
}

pub fn _rar_length(rar_path: &str) -> Result<usize, unrar::error::UnrarError> {
    let archive =
        Archive::new(rar_path)
            .open_for_listing()?;
    let len = archive.into_iter().filter(|e| e.is_ok()).map(|e|e.unwrap()).collect::<Vec<_>>().len();
    Ok(len)
}


pub fn _zip_length(zip_path: &str) -> Result<usize, io::Error> {
    let file = File::open(zip_path)?;
    let archive = ZipArchive::new(file)?;
    Ok(archive.len())
}