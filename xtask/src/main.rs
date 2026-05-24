use std::{
    env,
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
};

const PACKAGES: &[&str] = &["retoc-rivals-cli", "repak-gui"];
const ARTIFACTS: &[ArtifactSpec] = &[
    ArtifactSpec {
        package: "retoc-rivals-cli",
        binary: "retoc-rivals-cli",
    },
    ArtifactSpec {
        package: "repak-gui",
        binary: "repak-gui",
    },
];

struct ArtifactSpec {
    package: &'static str,
    binary: &'static str,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("xtask failed: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let Some(task) = args.next() else {
        print_usage();
        return Ok(());
    };

    match task.as_str() {
        "standalone-artifacts" | "self-contained-artifacts" => {
            let targets = collect_targets(args.collect())?;
            build_self_contained_artifacts(&targets)?;
        }
        "help" | "--help" | "-h" => print_usage(),
        other => return Err(format!("unknown task `{other}`").into()),
    }

    Ok(())
}

fn print_usage() {
    eprintln!(
        "usage: cargo run -p xtask -- standalone-artifacts [--target <triple> ... | <triple> ...]"
    );
}

fn collect_targets(args: Vec<String>) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut targets = Vec::new();
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        if arg == "--target" {
            let Some(target) = iter.next() else {
                return Err("--target requires a target triple".into());
            };
            targets.push(target);
        } else {
            targets.push(arg);
        }
    }

    if targets.is_empty() {
        for key in ["CARGO_DIST_TARGET", "TARGET", "CARGO_BUILD_TARGET"] {
            if let Ok(target) = env::var(key) {
                if !target.trim().is_empty() {
                    targets.push(target);
                    break;
                }
            }
        }
    }

    if targets.is_empty() {
        targets.push(host_target()?);
    }

    targets.sort();
    targets.dedup();
    Ok(targets)
}

fn build_self_contained_artifacts(targets: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    for target in targets {
        ensure_native_target(target)?;
        println!("building self-contained release artifacts for {target}");
        run_cargo_build(target)?;

        for archive in package_target(target)? {
            println!("created {}", archive.display());
        }
    }

    Ok(())
}

fn ensure_native_target(target: &str) -> Result<(), Box<dyn std::error::Error>> {
    let target_os = if target.contains("windows") {
        "windows"
    } else if target.contains("linux") {
        "linux"
    } else if target.contains("apple") || target.contains("darwin") {
        "macos"
    } else {
        return Err(format!("unsupported target `{target}` for standalone artifacts").into());
    };

    let host_os = env::consts::OS;
    if host_os == target_os {
        Ok(())
    } else {
        Err(format!(
            "refusing to cross-compile standalone artifact `{target}` on `{host_os}`; run this xtask on a native `{target_os}` runner"
        )
        .into())
    }
}

fn run_cargo_build(target: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut command = Command::new("cargo");
    command
        .arg("build")
        .arg("--profile")
        .arg("dist")
        .arg("--target")
        .arg(target)
        .env("RETOC_KAWAII_BINDING_SELF_CONTAINED", "true");

    for package in PACKAGES {
        command.arg("--package").arg(package);
    }

    run_command(command)
}

fn package_target(target: &str) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let target_dir = Path::new("target");
    let dist_dir = target_dir.join("standalone-dist");
    let build_dir = target_dir.join(target).join("dist");
    let mut archives = Vec::new();

    fs::create_dir_all(&dist_dir)?;

    for artifact in ARTIFACTS {
        let package_name = format!("{}-{target}-self-contained", artifact.package);
        let stage_dir = dist_dir.join(&package_name);

        if stage_dir.exists() {
            fs::remove_dir_all(&stage_dir)?;
        }
        fs::create_dir_all(&stage_dir)?;

        let binary_name = binary_name(artifact.binary, target);
        copy_file(build_dir.join(&binary_name), stage_dir.join(&binary_name))?;

        copy_if_exists("README.md", &stage_dir)?;
        copy_if_exists("CHANGELOG.md", &stage_dir)?;
        copy_if_exists("LICENSE-MIT", &stage_dir)?;
        copy_if_exists("LICENSE-APACHE", &stage_dir)?;
        copy_if_exists("LICENSE-GPL", &stage_dir)?;

        remove_archive_variants(&dist_dir, &package_name)?;
        let archive = if target.contains("windows") {
            zip_dir(&dist_dir, &package_name)?
        } else {
            tar_xz_dir(&dist_dir, &package_name)?
        };
        archives.push(archive);
    }

    Ok(archives)
}

fn copy_file(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
    let from = from.as_ref();
    let to = to.as_ref();
    fs::copy(from, to).map(|_| ()).map_err(|err| {
        io::Error::new(
            err.kind(),
            format!(
                "failed to copy {} to {}: {err}",
                from.display(),
                to.display()
            ),
        )
    })
}

fn copy_if_exists(path: impl AsRef<Path>, stage_dir: &Path) -> io::Result<()> {
    let path = path.as_ref();
    if path.exists() {
        copy_file(
            path,
            stage_dir.join(path.file_name().unwrap_or_else(|| OsStr::new("file"))),
        )?;
    }
    Ok(())
}

fn binary_name(binary: &str, target: &str) -> String {
    if target.contains("windows") {
        format!("{binary}.exe")
    } else {
        binary.to_owned()
    }
}

fn remove_archive_variants(dist_dir: &Path, package_name: &str) -> io::Result<()> {
    for extension in ["tar.gz", "tar.xz", "zip"] {
        let archive = dist_dir.join(format!("{package_name}.{extension}"));
        if archive.exists() {
            fs::remove_file(archive)?;
        }
    }
    Ok(())
}

fn tar_xz_dir(dist_dir: &Path, package_name: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let archive = dist_dir.join(format!("{package_name}.tar.xz"));
    let mut command = Command::new("tar");
    command
        .arg("-cJf")
        .arg(&archive)
        .arg("-C")
        .arg(dist_dir)
        .arg(package_name);
    run_command(command)?;
    Ok(archive)
}

fn zip_dir(dist_dir: &Path, package_name: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let archive = dist_dir.join(format!("{package_name}.zip"));
    let source = dist_dir.join(package_name);
    let script_path = dist_dir.join(format!(".zip-{package_name}.ps1"));
    let script = r#"
param(
    [Parameter(Mandatory=$true)]
    [string]$Source,

    [Parameter(Mandatory=$true)]
    [string]$Destination
)

$children = Get-ChildItem -LiteralPath $Source -Force
if (Test-Path -LiteralPath $Destination) {
    Remove-Item -LiteralPath $Destination -Force
}
Compress-Archive -LiteralPath $children.FullName -DestinationPath $Destination -Force
"#;

    fs::write(&script_path, script)?;

    if command_exists("powershell") {
        let mut command = Command::new("powershell");
        command
            .arg("-NoLogo")
            .arg("-NoProfile")
            .arg("-File")
            .arg(&script_path)
            .arg("-Source")
            .arg(&source)
            .arg("-Destination")
            .arg(&archive);
        let result = run_command(command);
        let _ = fs::remove_file(&script_path);
        result?;
    } else if command_exists("pwsh") {
        let mut command = Command::new("pwsh");
        command
            .arg("-NoLogo")
            .arg("-NoProfile")
            .arg("-File")
            .arg(&script_path)
            .arg("-Source")
            .arg(&source)
            .arg("-Destination")
            .arg(&archive);
        let result = run_command(command);
        let _ = fs::remove_file(&script_path);
        result?;
    } else {
        let _ = fs::remove_file(&script_path);
        return Err(
            "Windows self-contained artifacts require powershell or pwsh to create the zip".into(),
        );
    }

    Ok(archive)
}

fn command_exists(name: &str) -> bool {
    Command::new(name)
        .arg("-NoLogo")
        .arg("-NoProfile")
        .arg("-Command")
        .arg("$PSVersionTable.PSVersion | Out-Null")
        .status()
        .is_ok_and(|status| status.success())
}

fn run_command(mut command: Command) -> Result<(), Box<dyn std::error::Error>> {
    let status = command.status().map_err(|err| {
        let program = command.get_program().to_string_lossy();
        format!("failed to start `{program}`: {err}")
    })?;
    ensure_success(command.get_program(), status)
}

fn ensure_success(program: &OsStr, status: ExitStatus) -> Result<(), Box<dyn std::error::Error>> {
    if status.success() {
        Ok(())
    } else {
        Err(format!("`{}` exited with {status}", program.to_string_lossy()).into())
    }
}

fn host_target() -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("rustc").arg("-vV").output()?;
    if !output.status.success() {
        return Err("failed to query rustc host target".into());
    }

    let stdout = String::from_utf8(output.stdout)?;
    stdout
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .map(str::to_owned)
        .ok_or_else(|| "rustc -vV did not report a host target".into())
}
