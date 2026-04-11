mod pack;
mod utils;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use comfy_table::{presets::UTF8_FULL, Cell, ContentArrangement, Table};
use glob::glob;
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use unreal_packaging_backend::{ExtractUtocRequest, RetocBackend, UnrealPackagingBackend};
use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[command(author, version, about = "CLI port of repak-gui helper flows")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Extract one or more .utoc files into sibling directories
    Extract {
        #[arg(required = true)]
        inputs: Vec<String>,
        #[arg(long, value_enum, default_value_t = BackendKind::Retoc)]
        backend: BackendKind,
        #[arg(long, default_value_t = false)]
        keep_going: bool,
        #[arg(long, default_value_t = false)]
        no_progress: bool,
    },
    /// Recursively find and extract every .utoc under a directory
    ExtractDir {
        search_dir: PathBuf,
        #[arg(long, value_enum, default_value_t = BackendKind::Retoc)]
        backend: BackendKind,
        #[arg(long, default_value_t = false)]
        keep_going: bool,
        #[arg(long, default_value_t = false)]
        no_progress: bool,
    },
    /// Pack one or more mod directories using repak-gui iostore behavior
    Pack {
        #[arg(required = true)]
        inputs: Vec<String>,
        #[arg(long, value_enum, default_value_t = BackendKind::Retoc)]
        backend: BackendKind,
        #[arg(long, default_value_t = false)]
        keep_going: bool,
        #[arg(long, default_value_t = false)]
        no_progress: bool,
        #[arg(long, default_value = "../../../")]
        mount_point: String,
        #[arg(long, default_value = "00000000")]
        path_hash_seed: String,
        #[arg(long, default_value = "oodle")]
        compression: String,
        #[arg(long, default_value_t = false)]
        fix_mesh: bool,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum BackendKind {
    Retoc,
}

#[derive(Debug)]
struct ExtractRow {
    input: PathBuf,
    output: PathBuf,
    backend: String,
    status: String,
    duration: Duration,
    detail: String,
}

#[derive(Debug)]
struct PackRow {
    input: PathBuf,
    mod_name: String,
    mod_type: String,
    files: usize,
    output_pak: String,
    output_utoc: String,
    backend: String,
    status: String,
    duration: Duration,
    detail: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Extract {
            inputs,
            backend,
            keep_going,
            no_progress,
        } => extract_many(&inputs, backend, keep_going, no_progress),
        Command::ExtractDir {
            search_dir,
            backend,
            keep_going,
            no_progress,
        } => extract_dir(&search_dir, backend, keep_going, no_progress),
        Command::Pack {
            inputs,
            backend,
            keep_going,
            no_progress,
            mount_point,
            path_hash_seed,
            compression,
            fix_mesh,
        } => pack_many(
            &inputs,
            backend,
            keep_going,
            no_progress,
            pack::PackOptions {
                mount_point,
                path_hash_seed,
                compression: pack::parse_compression(&compression)?,
                fix_mesh,
            },
        ),
    }
}

fn extract_many(
    input_patterns: &[String],
    backend: BackendKind,
    keep_going: bool,
    no_progress: bool,
) -> Result<()> {
    let inputs = expand_patterns(input_patterns, InputKind::File)?;
    if inputs.is_empty() {
        bail!("no input files resolved from patterns");
    }

    let backend = create_backend(backend);
    let progress = create_progress(inputs.len(), no_progress, "Extracting .utoc files");
    let mut rows = Vec::new();
    let mut failures = 0usize;

    for input in inputs {
        let started = Instant::now();
        let row = match build_extract_output(&input)
            .and_then(|output| run_extract(backend.as_ref(), &input, &output).map(|_| output))
        {
            Ok(output) => ExtractRow {
                input,
                output,
                backend: backend.name().to_string(),
                status: "ok".to_string(),
                duration: started.elapsed(),
                detail: String::new(),
            },
            Err(error) => {
                failures += 1;
                ExtractRow {
                    input,
                    output: PathBuf::new(),
                    backend: backend.name().to_string(),
                    status: "fail".to_string(),
                    duration: started.elapsed(),
                    detail: error.to_string(),
                }
            }
        };
        rows.push(row);
        if let Some(pb) = &progress {
            pb.inc(1);
        }
        if failures > 0 && !keep_going {
            break;
        }
    }

    if let Some(pb) = progress {
        pb.finish_and_clear();
    }
    print_extract_table("Extract Summary", &rows);

    if failures > 0 {
        bail!("extract finished with {failures} failure(s)");
    }
    Ok(())
}

fn extract_dir(
    search_dir: &Path,
    backend: BackendKind,
    keep_going: bool,
    no_progress: bool,
) -> Result<()> {
    let utocs = WalkDir::new(search_dir)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().extension().and_then(|s| s.to_str()) == Some("utoc"))
        .map(|entry| entry.path().to_path_buf())
        .collect::<Vec<_>>();

    if utocs.is_empty() {
        bail!("no .utoc files found under {}", search_dir.display());
    }

    let backend = create_backend(backend);
    let progress = create_progress(
        utocs.len(),
        no_progress,
        "Extracting discovered .utoc files",
    );
    let mut rows = Vec::new();
    let mut failures = 0usize;

    for utoc in utocs {
        let started = Instant::now();
        let row = match build_extract_dir_output(&utoc)
            .and_then(|output| run_extract(backend.as_ref(), &utoc, &output).map(|_| output))
        {
            Ok(output) => ExtractRow {
                input: utoc,
                output,
                backend: backend.name().to_string(),
                status: "ok".to_string(),
                duration: started.elapsed(),
                detail: String::new(),
            },
            Err(error) => {
                failures += 1;
                ExtractRow {
                    input: utoc,
                    output: PathBuf::new(),
                    backend: backend.name().to_string(),
                    status: "fail".to_string(),
                    duration: started.elapsed(),
                    detail: error.to_string(),
                }
            }
        };
        rows.push(row);
        if let Some(pb) = &progress {
            pb.inc(1);
        }
        if failures > 0 && !keep_going {
            break;
        }
    }

    if let Some(pb) = progress {
        pb.finish_and_clear();
    }
    print_extract_table("Extract-Dir Summary", &rows);

    if failures > 0 {
        bail!("extract-dir finished with {failures} failure(s)");
    }
    Ok(())
}

fn pack_many(
    input_patterns: &[String],
    backend: BackendKind,
    keep_going: bool,
    no_progress: bool,
    pack_options: pack::PackOptions,
) -> Result<()> {
    let inputs = expand_patterns(input_patterns, InputKind::Directory)?;
    if inputs.is_empty() {
        bail!("no input directories resolved from patterns");
    }

    let backend = create_backend(backend);
    let progress = create_progress(inputs.len(), no_progress, "Packing mods");
    let mut rows = Vec::new();
    let mut failures = 0usize;

    for input in inputs {
        let started = Instant::now();
        match pack::pack_one_directory(&input, &pack_options, backend.as_ref()) {
            Ok(result) => rows.push(PackRow {
                input: result.input,
                mod_name: result.mod_name,
                mod_type: result.mod_type,
                files: result.total_files,
                output_pak: result.output_pak.display().to_string(),
                output_utoc: result
                    .output_utoc
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "-".to_string()),
                backend: result.backend.to_string(),
                status: "ok".to_string(),
                duration: started.elapsed(),
                detail: String::new(),
            }),
            Err(error) => {
                failures += 1;
                rows.push(PackRow {
                    input,
                    mod_name: "-".to_string(),
                    mod_type: "-".to_string(),
                    files: 0,
                    output_pak: "-".to_string(),
                    output_utoc: "-".to_string(),
                    backend: backend.name().to_string(),
                    status: "fail".to_string(),
                    duration: started.elapsed(),
                    detail: error.to_string(),
                });
            }
        }

        if let Some(pb) = &progress {
            pb.inc(1);
        }
        if failures > 0 && !keep_going {
            break;
        }
    }

    if let Some(pb) = progress {
        pb.finish_and_clear();
    }
    print_pack_table("Pack Summary", &rows);

    if failures > 0 {
        bail!("pack finished with {failures} failure(s)");
    }
    Ok(())
}

fn build_extract_output(input: &Path) -> Result<PathBuf> {
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .with_context(|| format!("invalid input file name: {}", input.display()))?;
    let parent = input
        .parent()
        .with_context(|| format!("input has no parent directory: {}", input.display()))?;
    Ok(parent.join(stem))
}

fn build_extract_dir_output(input: &Path) -> Result<PathBuf> {
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .with_context(|| format!("invalid input file name: {}", input.display()))?;
    let parent = input
        .parent()
        .with_context(|| format!("input has no parent directory: {}", input.display()))?;
    Ok(parent.join(format!("unpacked_{stem}")))
}

fn run_extract(backend: &dyn UnrealPackagingBackend, input: &Path, output: &Path) -> Result<()> {
    std::fs::create_dir_all(output)
        .with_context(|| format!("failed to create {}", output.display()))?;
    backend.extract_utoc(&ExtractUtocRequest {
        input_utoc: input.to_path_buf(),
        output_dir: output.to_path_buf(),
        verbose: true,
    })
}

fn create_backend(kind: BackendKind) -> Box<dyn UnrealPackagingBackend> {
    match kind {
        BackendKind::Retoc => Box::new(RetocBackend::default()),
    }
}

fn create_progress(len: usize, disabled: bool, message: &str) -> Option<ProgressBar> {
    if disabled || len <= 1 {
        return None;
    }
    let pb = ProgressBar::new(len as u64);
    pb.set_style(
        ProgressStyle::with_template("[{elapsed_precise}] [{wide_bar}] {pos}/{len} {msg}")
            .expect("valid progress style"),
    );
    pb.set_message(message.to_string());
    Some(pb)
}

#[derive(Debug, Clone, Copy)]
enum InputKind {
    File,
    Directory,
}

fn expand_patterns(patterns: &[String], input_kind: InputKind) -> Result<Vec<PathBuf>> {
    let mut unique = BTreeSet::new();
    for pattern in patterns {
        let mut matched = false;
        if has_glob_token(pattern) {
            for entry in
                glob(pattern).with_context(|| format!("invalid glob pattern `{pattern}`"))?
            {
                matched = true;
                let path = entry.with_context(|| format!("invalid path from glob `{pattern}`"))?;
                unique.insert(path);
            }
            if !matched {
                bail!("pattern matched nothing: `{pattern}`");
            }
        } else {
            unique.insert(PathBuf::from(pattern));
        }
    }

    let resolved = unique
        .into_iter()
        .filter(|path| match input_kind {
            InputKind::File => path.is_file(),
            InputKind::Directory => path.is_dir(),
        })
        .collect::<Vec<_>>();

    if resolved.is_empty() {
        bail!("no valid paths after glob expansion");
    }
    Ok(resolved)
}

fn has_glob_token(value: &str) -> bool {
    value.contains('*') || value.contains('?') || value.contains('[') || value.contains('{')
}

fn print_extract_table(title: &str, rows: &[ExtractRow]) {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("Input"),
            Cell::new("Output"),
            Cell::new("Backend"),
            Cell::new("Status"),
            Cell::new("Time(ms)"),
            Cell::new("Detail"),
        ]);

    for row in rows {
        table.add_row(vec![
            Cell::new(row.input.display().to_string()),
            Cell::new(if row.output.as_os_str().is_empty() {
                "-".to_string()
            } else {
                row.output.display().to_string()
            }),
            Cell::new(&row.backend),
            Cell::new(&row.status),
            Cell::new(row.duration.as_millis()),
            Cell::new(&row.detail),
        ]);
    }

    println!("{title}");
    println!("{table}");
}

fn print_pack_table(title: &str, rows: &[PackRow]) {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("Input"),
            Cell::new("Name"),
            Cell::new("Type"),
            Cell::new("Files"),
            Cell::new("Pak"),
            Cell::new("Utoc"),
            Cell::new("Backend"),
            Cell::new("Status"),
            Cell::new("Time(ms)"),
            Cell::new("Detail"),
        ]);

    for row in rows {
        table.add_row(vec![
            Cell::new(row.input.display().to_string()),
            Cell::new(&row.mod_name),
            Cell::new(&row.mod_type),
            Cell::new(row.files),
            Cell::new(&row.output_pak),
            Cell::new(&row.output_utoc),
            Cell::new(&row.backend),
            Cell::new(&row.status),
            Cell::new(row.duration.as_millis()),
            Cell::new(&row.detail),
        ]);
    }

    println!("{title}");
    println!("{table}");
}
