use crate::utils::{collect_files, get_current_pak_characteristics};
use anyhow::{bail, Context, Result};
use path_slash::PathExt;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use repak::{Compression, Version};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::LazyLock;
use uasset_mesh_patch_rivals::{Logger, PatchFixer};
use unreal_packaging_backend::{
    PackDirectoryRequest, UnrealEngineVersion, UnrealPackagingBackend, DEFAULT_AES_KEY_HEX,
};

const MOD_NAME_SUFFIX: &str = "_9999999_P";
const DEFAULT_MESH_DIRS: &[&str] = &["Meshes", "Meshs"];

#[derive(Debug, Clone)]
pub struct PackOptions {
    pub mount_point: String,
    pub path_hash_seed: String,
    pub compression: Compression,
    pub fix_mesh: bool,
}

#[derive(Debug, Clone)]
struct InstallableMod {
    mod_name: String,
    mod_type: String,
    fix_mesh: bool,
    path_hash_seed: String,
    mount_point: String,
    compression: Compression,
    total_files: usize,
}

#[derive(Debug, Clone)]
pub struct PackResult {
    pub input: PathBuf,
    pub mod_name: String,
    pub mod_type: String,
    pub total_files: usize,
    pub output_pak: PathBuf,
    pub output_utoc: Option<PathBuf>,
    pub backend: &'static str,
}

struct PrintLogger;

impl Logger for PrintLogger {
    fn log<S: Into<String>>(&self, buf: S) {
        println!("[mesh] {}", Into::<String>::into(buf));
    }
}

static MESH_DIRECTORY_NAMES: LazyLock<Vec<String>> = LazyLock::new(|| {
    let path = Path::new("mesh_dir_list.txt");
    let mut names = DEFAULT_MESH_DIRS
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();

    if let Ok(contents) = fs::read_to_string(path) {
        for line in contents.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() && !names.iter().any(|name| name == trimmed) {
                names.push(trimmed.to_string());
            }
        }
    }

    names.sort();
    names.dedup();

    if let Ok(mut file) = File::create(path) {
        for name in &names {
            let _ = writeln!(file, "{name}");
        }
    }

    names
});

pub fn parse_compression(value: &str) -> Result<Compression> {
    Compression::from_str(value)
        .or_else(|_| Compression::from_str(&capitalize(value)))
        .with_context(|| format!("unsupported compression `{value}`"))
}

pub fn pack_one_directory(
    input: &Path,
    options: &PackOptions,
    backend: &dyn UnrealPackagingBackend,
) -> Result<PackResult> {
    if !input.exists() {
        bail!("input does not exist: {}", input.display());
    }
    if !input.is_dir() {
        bail!(
            "repak-cli2 pack expects directories only, got: {}",
            input.display()
        );
    }

    let installable = build_installable_mod(input, options)?;
    let output_dir = input
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    let packed_files_count = AtomicI32::new(0);
    let (output_pak, output_utoc) =
        convert_directory_to_iostore(&installable, output_dir, input.to_path_buf(), backend, &packed_files_count)
            .with_context(|| format!("failed to pack {}", input.display()))?;

    Ok(PackResult {
        input: input.to_path_buf(),
        mod_name: installable.mod_name,
        mod_type: installable.mod_type,
        total_files: installable.total_files,
        output_pak,
        output_utoc,
        backend: backend.name(),
    })
}

fn build_installable_mod(input: &Path, options: &PackOptions) -> Result<InstallableMod> {
    let mut files = Vec::new();
    collect_files(&mut files, input)
        .with_context(|| format!("failed to collect files in {}", input.display()))?;

    let mod_type = get_current_pak_characteristics(
        files.iter()
            .map(|path| normalize_for_classification(input, path))
            .collect(),
    );

    let mod_name = input
        .file_name()
        .and_then(|s| s.to_str())
        .with_context(|| format!("invalid directory name: {}", input.display()))?
        .to_string();

    Ok(InstallableMod {
        mod_name,
        mod_type,
        fix_mesh: options.fix_mesh,
        path_hash_seed: options.path_hash_seed.clone(),
        mount_point: options.mount_point.clone(),
        compression: options.compression,
        total_files: files.len(),
    })
}

fn convert_directory_to_iostore(
    pak: &InstallableMod,
    mod_dir: PathBuf,
    to_pak_dir: PathBuf,
    backend: &dyn UnrealPackagingBackend,
    packed_files_count: &AtomicI32,
) -> Result<(PathBuf, Option<PathBuf>)> {
    if pak.mod_type == "Audio" || pak.mod_type == "Movies" {
        let output_pak = repak_dir(pak, to_pak_dir, mod_dir, packed_files_count)?;
        return Ok((output_pak, None));
    }

    let normalized_mod_name = ensure_mod_name_suffix(&pak.mod_name);
    let pak_name = format!("{normalized_mod_name}.pak");
    let utoc_name = format!("{normalized_mod_name}.utoc");
    let output_pak = mod_dir.join(pak_name);
    let output_utoc = mod_dir.join(utoc_name);

    let mut paths = Vec::new();
    collect_files(&mut paths, &to_pak_dir)
        .with_context(|| format!("failed to collect files in {}", to_pak_dir.display()))?;

    if pak.fix_mesh {
        mesh_patch(&mut paths, &to_pak_dir)?;
    }

    backend.pack_directory_to_iostore(&PackDirectoryRequest {
        input_dir: to_pak_dir.clone(),
        output_utoc: output_utoc.clone(),
        engine_version: UnrealEngineVersion::UE5_3,
    })?;

    let output_file = File::create(&output_pak)
        .with_context(|| format!("failed to create pak {}", output_pak.display()))?;

    let rel_paths = paths
        .par_iter()
        .map(|path| slash_relative(&to_pak_dir, path))
        .collect::<Result<Vec<_>>>()?;

    let builder = repak::PakBuilder::new()
        .compression(vec![pak.compression])
        .key(repak::utils::AesKey::from_str(DEFAULT_AES_KEY_HEX)?.0);

    let mut pak_writer = builder.writer(
        BufWriter::new(output_file),
        Version::V11,
        pak.mount_point.clone(),
        Some(parse_path_hash_seed(&pak.path_hash_seed)?),
    );
    let entry_builder = pak_writer.entry_builder();
    let rel_paths_bytes = rel_paths.join("\n").into_bytes();
    let entry = entry_builder.build_entry(true, rel_paths_bytes, "chunknames")?;

    pak_writer.write_entry("chunknames".to_string(), entry)?;
    pak_writer.write_index()?;
    packed_files_count.fetch_add(1, Ordering::SeqCst);

    Ok((output_pak, Some(output_utoc)))
}

fn repak_dir(
    pak: &InstallableMod,
    to_pak_dir: PathBuf,
    mod_dir: PathBuf,
    packed_files_count: &AtomicI32,
) -> Result<PathBuf> {
    let pak_name = format!("{}.pak", ensure_mod_name_suffix(&pak.mod_name));
    let output_pak = mod_dir.join(pak_name);
    let output_file = File::create(&output_pak)
        .with_context(|| format!("failed to create pak {}", output_pak.display()))?;

    let mut paths = Vec::new();
    collect_files(&mut paths, &to_pak_dir)
        .with_context(|| format!("failed to collect files in {}", to_pak_dir.display()))?;

    if pak.fix_mesh {
        mesh_patch(&mut paths, &to_pak_dir)?;
    }

    paths.sort();

    let builder = repak::PakBuilder::new()
        .compression(vec![pak.compression])
        .key(repak::utils::AesKey::from_str(DEFAULT_AES_KEY_HEX)?.0);

    let mut pak_writer = builder.writer(
        BufWriter::new(output_file),
        Version::V11,
        pak.mount_point.clone(),
        Some(parse_path_hash_seed(&pak.path_hash_seed)?),
    );
    let entry_builder = pak_writer.entry_builder();

    let entries = paths
        .par_iter()
        .map(|path| {
            let rel = slash_relative(&to_pak_dir, path)?;
            let bytes =
                fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
            let entry = entry_builder.build_entry(true, bytes, &rel)?;
            Ok::<_, anyhow::Error>((rel, entry))
        })
        .collect::<Result<Vec<_>>>()?;

    let mut rel_paths = Vec::with_capacity(entries.len());
    for (path, entry) in entries {
        pak_writer.write_entry(path.clone(), entry)?;
        rel_paths.push(path);
        packed_files_count.fetch_add(1, Ordering::SeqCst);
    }

    let rel_paths_bytes = rel_paths.join("\n").into_bytes();
    let entry = entry_builder.build_entry(true, rel_paths_bytes, "chunknames")?;
    pak_writer.write_entry("chunknames".to_string(), entry)?;
    pak_writer.write_index()?;

    Ok(output_pak)
}

fn mesh_patch(paths: &mut Vec<PathBuf>, mod_dir: &Path) -> Result<()> {
    let uasset_files = paths
        .iter()
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("uasset"))
                && path.components().any(|component| {
                    let name = component.as_os_str().to_string_lossy().to_lowercase();
                    MESH_DIRECTORY_NAMES
                        .iter()
                        .any(|dir_name| dir_name.to_lowercase() == name)
                })
        })
        .cloned()
        .collect::<Vec<_>>();

    if uasset_files.is_empty() {
        return Ok(());
    }

    let patched_cache_file = mod_dir.join("patched_files");
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&patched_cache_file)
        .with_context(|| format!("failed to open {}", patched_cache_file.display()))?;

    let patched_files = BufReader::new(&file)
        .lines()
        .collect::<std::io::Result<Vec<_>>>()
        .context("failed to read patched file cache")?;

    let mut cache_writer = BufWriter::new(&file);
    paths.push(patched_cache_file);

    let mut fixer = PatchFixer {
        logger: PrintLogger,
    };

    'outer: for uasset_file in &uasset_files {
        let mut sizes = Vec::new();
        let mut offsets = Vec::new();
        let dir_path = uasset_file
            .parent()
            .with_context(|| format!("uasset has no parent: {}", uasset_file.display()))?;
        let uexp_file = dir_path.join(
            uasset_file
                .file_name()
                .and_then(|s| s.to_str())
                .with_context(|| format!("invalid file name: {}", uasset_file.display()))?
                .replace(".uasset", ".uexp"),
        );

        if !uexp_file.exists() {
            bail!("missing matching .uexp for {}", uasset_file.display());
        }

        let rel_uasset = slash_relative(mod_dir, uasset_file)?;
        let rel_uexp = slash_relative(mod_dir, &uexp_file)?;

        for already_patched in &patched_files {
            if already_patched == &rel_uasset || already_patched == &rel_uexp {
                println!("Skipping already patched {}", rel_uasset);
                continue 'outer;
            }
        }

        let uexp_backup = dir_path.join(format!(
            "{}.bak",
            uexp_file
                .file_name()
                .and_then(|s| s.to_str())
                .with_context(|| format!("invalid file name: {}", uexp_file.display()))?
        ));
        let uasset_backup = dir_path.join(format!(
            "{}.bak",
            uasset_file
                .file_name()
                .and_then(|s| s.to_str())
                .with_context(|| format!("invalid file name: {}", uasset_file.display()))?
        ));
        fs::copy(&uexp_file, &uexp_backup)?;
        fs::copy(uasset_file, &uasset_backup)?;

        let mut reader = BufReader::new(File::open(uasset_file)?);
        let (export_count, export_offset) = fixer.read_uasset(&mut reader)?;
        fixer.read_exports(
            &mut reader,
            &mut sizes,
            &mut offsets,
            export_offset,
            export_count,
        )?;
        drop(reader);

        let backup_file_size = fs::metadata(uasset_file)?.len();
        let temp_file = dir_path.join(format!(
            "{}.temp",
            uexp_file
                .file_name()
                .and_then(|s| s.to_str())
                .with_context(|| format!("invalid file name: {}", uexp_file.display()))?
        ));

        let mut input = BufReader::new(File::open(&uexp_backup)?);
        let mut output = BufWriter::new(File::create(&temp_file)?);

        match fixer.read_uexp(
            &mut input,
            backup_file_size,
            &uexp_backup.to_string_lossy(),
            &mut output,
            &offsets,
        ) {
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::Other => {
                let _ = fs::remove_file(&temp_file);
                continue 'outer;
            }
            Err(error) => return Err(error.into()),
        }

        fs::copy(&temp_file, &uexp_file)?;
        unsafe {
            fixer.clean_uasset(uasset_file.clone(), &sizes)?;
        }

        writeln!(&mut cache_writer, "{rel_uasset}")?;
        writeln!(&mut cache_writer, "{rel_uexp}")?;
        cache_writer.flush()?;
        fs::remove_file(&temp_file)?;
    }

    Ok(())
}

fn normalize_for_classification(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .ok()
        .and_then(|value| value.to_slash())
        .map(|value| value.to_string())
        .unwrap_or_else(|| path.to_string_lossy().replace('\\', "/"))
}

fn slash_relative(root: &Path, path: &Path) -> Result<String> {
    path.strip_prefix(root)
        .with_context(|| format!("path {} was not under {}", path.display(), root.display()))?
        .to_slash()
        .map(|value| value.to_string())
        .ok_or_else(|| anyhow::anyhow!("failed to convert path to slash format: {}", path.display()))
}

fn parse_path_hash_seed(value: &str) -> Result<u64> {
    if let Ok(parsed) = value.parse::<u64>() {
        return Ok(parsed);
    }

    u64::from_str_radix(value.trim_start_matches("0x"), 16)
        .with_context(|| format!("invalid path hash seed `{value}`"))
}

fn ensure_mod_name_suffix(name: &str) -> String {
    if name.ends_with(MOD_NAME_SUFFIX) {
        name.to_string()
    } else {
        format!("{name}{MOD_NAME_SUFFIX}")
    }
}

fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}
