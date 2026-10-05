use std::{
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use anyhow::{Context as _, Result, bail};
use cargo_metadata::MetadataCommand;
use clap::Parser;
use serde_json::Value;

/// Default target triple for eBPF programs.
const DEFAULT_TARGET: &str = "bpfel-unknown-none";

/// Build one eBPF package and copy the resulting ELF object to `--output`, from
/// where the tests embed it with `aya::include_bytes_aligned!`.
#[derive(Debug, Parser)]
pub struct BuildEbpfOptions {
    /// The eBPF package to build.
    #[clap(long = "package", short = 'p', value_name = "PKG")]
    pub package: String,

    /// `Cargo.toml` of the workspace containing the package. Defaults to the
    /// workspace of the current directory.
    #[clap(long, value_name = "PATH")]
    pub manifest_path: Option<PathBuf>,

    /// Path the ELF object is copied to.
    #[clap(long = "output", short = 'o', value_name = "FILE")]
    pub output: PathBuf,

    /// Target triple for the eBPF programs.
    #[clap(long, default_value = DEFAULT_TARGET)]
    pub target: String,

    /// Rust toolchain to use (e.g. "nightly", "nightly-2024-01-01").
    /// Must be nightly because of -Z build-std. Defaults to the toolchain
    /// pinned in rust-toolchain.toml, the same one the userspace build uses.
    #[clap(long)]
    pub toolchain: Option<String>,
}

pub(crate) fn run(opts: BuildEbpfOptions) -> Result<()> {
    let pkg = opts.package.as_str();

    // `cargo metadata` gives us the workspace root (so paths are always
    // correct regardless of where xtask is invoked from) and lets us validate
    // the requested package name up front.
    let mut metadata_cmd = MetadataCommand::new();
    if let Some(manifest_path) = &opts.manifest_path {
        metadata_cmd.manifest_path(manifest_path);
    }
    let metadata = metadata_cmd
        .no_deps()
        .exec()
        .context("failed to run `cargo metadata`")?;

    let workspace_root = metadata.workspace_root.as_std_path().to_path_buf();

    if !metadata.packages.iter().any(|p| p.name.as_str() == pkg) {
        bail!("package `{pkg}` was not found in the workspace");
    }

    println!("==> Building eBPF package `{pkg}`");

    let artifacts = build_package(
        pkg,
        &opts.target,
        opts.toolchain.as_deref(),
        &workspace_root,
    )
    .with_context(|| format!("failed to build package `{pkg}`"))?;

    let artifact = match artifacts.as_slice() {
        [artifact] => artifact,
        [] => bail!(
            "package `{pkg}` built successfully but produced no artifacts.\n\
             \n\
             Possible causes:\n\
             • The package has no [[bin]] target (add one in its Cargo.toml)\n\
             • The binary name differs from the package name\n\
             • The build was cached — try `cargo clean -p {pkg}` and retry"
        ),
        _ => bail!(
            "package `{pkg}` produced {} artifacts, expected exactly one [[bin]] target",
            artifacts.len()
        ),
    };

    copy_artifact(artifact, &opts.output).context("failed to copy eBPF artifact")?;

    println!("\nDone. `{pkg}` written to `{}`.", opts.output.display());
    Ok(())
}

// ---------------------------------------------------------------------------
// Build
// ---------------------------------------------------------------------------

/// Compile one package for the eBPF target and return the **executable**
/// paths that cargo reports via its JSON message stream.
///
/// We use the `executable` field of `compiler-artifact` messages rather than
/// `filenames`, because `filenames` also contains `.d` depfiles and `.rlib`
/// intermediates.  The `executable` field is set only for `[[bin]]` targets
/// and is exactly the ELF file we want to hand to Aya.
fn build_package(
    pkg: &str,
    target: &str,
    toolchain: Option<&str>,
    workspace_root: &Path,
) -> Result<Vec<PathBuf>> {
    // Build the argument list.  We keep it as Vec<String> so we can push
    // conditionally without lifetime headaches.
    let mut args: Vec<String> = toolchain.map(|t| format!("+{t}")).into_iter().collect();
    args.extend([
        "build".into(),
        "-p".into(),
        pkg.into(),
        "--profile".into(),
        "ebpf".into(),
        "--target".into(),
        target.into(),
        "-Z".into(),
        "build-std=core".into(),
        // json-render-diagnostics: JSON on stdout, human-readable diagnostics
        // on stderr — best of both worlds.
        "--message-format=json-render-diagnostics".into(),
    ]);

    let mut child = Command::new("cargo")
        .args(&args)
        .current_dir(workspace_root)
        .stderr(Stdio::inherit()) // compiler errors stream straight to terminal
        .stdout(Stdio::piped())
        .spawn()
        .context("failed to spawn `cargo build`")?;

    let stdout = child
        .stdout
        .take()
        .expect("stdout was piped but unavailable");

    let reader = BufReader::new(stdout);
    let mut artifacts: Vec<PathBuf> = Vec::new();

    for line in reader.lines() {
        let line = line.context("error reading cargo stdout")?;

        let v: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue, // skip non-JSON lines (rare with json-render-diagnostics)
        };

        match v["reason"].as_str() {
            Some("compiler-artifact") => {
                // Guard: only collect artifacts that belong to the package we
                // asked to build.  The `target.name` field holds the crate
                // name, which is stable and unambiguous.  We also check
                // `package_id` as a fallback for workspaces where the target name
                // was customised.
                let target_name = v["target"]["name"].as_str().unwrap_or("");
                let package_id = v["package_id"].as_str().unwrap_or("");
                let is_our_pkg = target_name == pkg
                    || package_id.starts_with(&format!("{pkg} "))
                    || package_id.starts_with(&format!("{pkg}@"));

                if !is_our_pkg {
                    continue;
                }

                // `executable` is present for [[bin]] targets and is the
                // single ELF file we want.  It is absent for rlib/cdylib etc.
                if let Some(exe) = v["executable"].as_str() {
                    artifacts.push(PathBuf::from(exe));
                }
            }

            Some("build-finished") => {
                if v["success"].as_bool() == Some(false) {
                    bail!("cargo reported build-finished with success=false");
                }
            }

            _ => {}
        }
    }

    let status = child.wait().context("failed to wait for `cargo build`")?;
    if !status.success() {
        bail!("`cargo build` exited with {status}");
    }

    Ok(artifacts)
}

// ---------------------------------------------------------------------------
// Copy
// ---------------------------------------------------------------------------

/// Copy `artifact` to `output`, creating its directory if needed. The file is
/// always rewritten, so its modification time marks the last build.
fn copy_artifact(artifact: &Path, output: &Path) -> Result<()> {
    if let Some(dir) = output.parent().filter(|d| !d.as_os_str().is_empty()) {
        fs::create_dir_all(dir).with_context(|| format!("failed to create `{}`", dir.display()))?;
    }

    fs::copy(artifact, output).with_context(|| {
        format!(
            "failed to copy `{}` to `{}`",
            artifact.display(),
            output.display()
        )
    })?;
    println!(
        "  copied  `{}` → `{}`",
        artifact.display(),
        output.display()
    );

    Ok(())
}
