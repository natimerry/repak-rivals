use std::{
    env,
    ffi::OsStr,
    fs,
    io,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
};

const BINARIES: &[&str] = &["retoc-rivals-cli", "repak-gui"];
const PACKAGES: &[&str] = &["retoc-rivals-cli", "repak-gui"];

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
        println!("building self-contained release artifacts for {target}");
        run_cargo_build(target)?;

        let archive = package_target(target)?;
        println!("created {}", archive.display());
    }

    Ok(())
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

fn package_target(target: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let target_dir = Path::new("target");
    let dist_dir = target_dir.join("standalone-dist");
    let package_name = format!("repak-rivals-{target}-self-contained");
    let stage_dir = dist_dir.join(&package_name);

    if stage_dir.exists() {
        fs::remove_dir_all(&stage_dir)?;
    }
    fs::create_dir_all(&stage_dir)?;

    let build_dir = target_dir.join(target).join("dist");
    for binary in BINARIES {
        let binary_name = binary_name(binary, target);
        copy_file(build_dir.join(&binary_name), stage_dir.join(&binary_name))?;
    }

    copy_if_exists("README.md", &stage_dir)?;
    copy_if_exists("CHANGELOG.md", &stage_dir)?;
    copy_if_exists("LICENSE-MIT", &stage_dir)?;
    copy_if_exists("LICENSE-APACHE", &stage_dir)?;
    copy_if_exists("LICENSE-GPL", &stage_dir)?;

    if target.contains("windows") {
        zip_dir(&dist_dir, &package_name)
    } else {
        tar_gz_dir(&dist_dir, &package_name)
    }
}

fn copy_file(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
    let from = from.as_ref();
    let to = to.as_ref();
    fs::copy(from, to).map(|_| ()).map_err(|err| {
        io::Error::new(
            err.kind(),
            format!("failed to copy {} to {}: {err}", from.display(), to.display()),
        )
    })
}

fn copy_if_exists(path: impl AsRef<Path>, stage_dir: &Path) -> io::Result<()> {
    let path = path.as_ref();
    if path.exists() {
        copy_file(path, stage_dir.join(path.file_name().unwrap_or_else(|| OsStr::new("file"))))?;
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

fn tar_gz_dir(dist_dir: &Path, package_name: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let archive = dist_dir.join(format!("{package_name}.tar.gz"));
    let mut command = Command::new("tar");
    command
        .arg("-czf")
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

    if command_exists("powershell") {
        let mut command = Command::new("powershell");
        command
            .arg("-NoLogo")
            .arg("-NoProfile")
            .arg("-Command")
            .arg("Compress-Archive -LiteralPath $args[0] -DestinationPath $args[1] -Force")
            .arg(source)
            .arg(&archive);
        run_command(command)?;
    } else if command_exists("pwsh") {
        let mut command = Command::new("pwsh");
        command
            .arg("-NoLogo")
            .arg("-NoProfile")
            .arg("-Command")
            .arg("Compress-Archive -LiteralPath $args[0] -DestinationPath $args[1] -Force")
            .arg(source)
            .arg(&archive);
        run_command(command)?;
    } else {
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
