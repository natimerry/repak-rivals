use colored::Colorize;
use log::info;
use path_slash::PathExt;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, ErrorKind, Write};
use std::path::PathBuf;
use uasset_mesh_patch_rivals::Logger;
use uasset_mesh_patch_rivals::PatchFixer;

struct PrintLogger;

impl Logger for PrintLogger {
    fn log<S: Into<String>>(&self, buf: S) {
        let s = Into::<String>::into(buf);
        info!("[Mesh Patcher] {}", s);
    }
}

pub fn mesh_patch(paths: &mut Vec<PathBuf>, mod_dir: &PathBuf) -> Result<(), repak::Error> {
    let uasset_files = paths
        .iter()
        .filter(|p| {
            p.extension().and_then(|ext| ext.to_str()) == Some("uasset")
                && ((p.to_str().unwrap().to_lowercase().starts_with("mesh"))
                    || (p.to_str().unwrap().to_lowercase().contains("meshes"))) // idk i didnt test this so im keeping this redundant check
        })
        .cloned()
        .collect::<Vec<PathBuf>>();

    let patched_cache_file = mod_dir.join("patched_files");
    info!("Patching files...");
    let file = OpenOptions::new()
        .read(true) // Allow reading
        .write(true) // Allow writing
        .create(true)
        .truncate(false) // Create the file if it doesn’t exist
        .open(&patched_cache_file)?;

    let patched_files = BufReader::new(&file)
        .lines()
        .map(|l| l.unwrap().clone())
        .collect::<Vec<_>>();

    let mut cache_writer = BufWriter::new(&file);

    paths.push(patched_cache_file);
    let print_logger = PrintLogger;
    let mut fixer = PatchFixer {
        logger: print_logger,
    };
    'outer: for uassetfile in &uasset_files {
        let mut sizes: Vec<i64> = vec![];
        let mut offsets: Vec<i64> = vec![];

        let dir_path = uassetfile.parent().unwrap();
        let uexp_file = dir_path.join(
            uassetfile
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .replace(".uasset", ".uexp"),
        );

        if !uexp_file.exists() {
            panic!("UEXP file doesnt exist");
            // damn
        }

        let rel_uasset = &uassetfile
            .strip_prefix(mod_dir)
            .expect("file not in input directory")
            .to_slash()
            .expect("failed to convert to slash path");

        let rel_uexp = &uexp_file
            .strip_prefix(mod_dir)
            .expect("file not in input directory")
            .to_slash()
            .expect("failed to convert to slash path");

        for i in &patched_files {
            if i.as_str() == *rel_uexp || i.as_str() == *rel_uasset {
                info!(
                    "Skipping {} (File has already been patched before)",
                    i.yellow()
                );
                continue 'outer;
            }
        }

        fs::copy(
            &uexp_file,
            dir_path.join(format!(
                "{}.bak",
                uexp_file.file_name().unwrap().to_str().unwrap()
            )),
        )?;
        fs::copy(
            uassetfile,
            dir_path.join(format!(
                "{}.bak",
                uassetfile.file_name().unwrap().to_str().unwrap()
            )),
        )?;

        info!("Processing {}", &uassetfile.to_str().unwrap().yellow());
        let mut rdr = BufReader::new(File::open(uassetfile.clone())?);
        let (exp_cnt, exp_offset) = fixer.read_uasset(&mut rdr)?;
        fixer.read_exports(&mut rdr, &mut sizes, &mut offsets, exp_offset, exp_cnt)?;

        let backup_file = format!("{}.bak", uexp_file.to_str().unwrap());
        let backup_file_size = fs::metadata(uassetfile)?.len();
        let tmpfile = format!("{}.temp", uexp_file.to_str().unwrap());

        drop(rdr);

        let mut r = BufReader::new(File::open(&backup_file)?);
        let mut o = BufWriter::new(File::create(&tmpfile)?);

        let exp_rd = fixer.read_uexp(&mut r, backup_file_size, &backup_file, &mut o, &offsets);
        match exp_rd {
            Ok(_) => {}
            Err(e) => match e.kind() {
                ErrorKind::InvalidData => {
                    panic!("{}", e.to_string())
                }
                ErrorKind::Other => {
                    fs::remove_file(&tmpfile)?;
                    continue 'outer;
                }
                _ => {
                    panic!("{}", e.to_string())
                }
            },
        }
        // fs::remove_file(&uexp_file)?;

        fs::copy(&tmpfile, &uexp_file)?;
        unsafe {
            fixer.clean_uasset(uassetfile.clone(), &sizes)?;
        }

        writeln!(&mut cache_writer, "{}", &rel_uasset)?;
        writeln!(&mut cache_writer, "{}", &rel_uexp)?;

        fs::remove_file(&tmpfile)?;
        cache_writer.flush()?;
    }

    info!("Done patching files!!");
    Ok(())
}
