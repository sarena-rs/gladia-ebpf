use std::{
    fmt::Write as _,
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use anyhow::{Context as _, Result, bail};
use cargo_metadata::MetadataCommand;
use clap::Parser;
use serde_json::Value;
use sha2::{Digest as _, Sha256};

/// Default target triple for eBPF programs.
const DEFAULT_TARGET: &str = "bpfel-unknown-none";

/// Build one eBPF package and generate a Rust file that embeds the resulting
/// object as an `EbpfObject` static.
#[derive(Debug, Parser)]
pub struct BuildEbpfOptions {
    /// The eBPF package to build.
    #[clap(long = "package", short = 'p', value_name = "PKG")]
    pub package: String,

    /// `Cargo.toml` of the workspace containing the package. Defaults to the
    /// workspace of the current directory.
    #[clap(long, value_name = "PATH")]
    pub manifest_path: Option<PathBuf>,

    /// Path of the generated Rust file. The ELF object is written next to it
    /// with the same file stem and an `.o` extension.
    #[clap(long = "output", short = 'o', value_name = "FILE")]
    pub output: PathBuf,

    /// Name of the generated static. Defaults to the package name in
    /// SCREAMING_SNAKE_CASE (`ebpf-programs` → `EBPF_PROGRAMS`).
    #[clap(long = "const", value_name = "NAME")]
    pub constant: Option<String>,

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

    let constant = opts
        .constant
        .clone()
        .unwrap_or_else(|| pkg.replace('-', "_").to_uppercase());

    embed_artifact(pkg, &constant, artifact, &opts.output)
        .context("failed to embed eBPF artifact")?;

    println!(
        "\nDone. `{pkg}` embedded as `{constant}` in `{}`.",
        opts.output.display()
    );
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
// Embed
// ---------------------------------------------------------------------------

/// Copy `artifact` next to `output` (same stem, `.o` extension) and write
/// `output` as a Rust file declaring a `pub static <constant>: EbpfObject`
/// that embeds it.
///
/// The generated file refers to `EbpfObject` unqualified, so it must be
/// `include!`d where that type is in scope. The object is referenced by a
/// path relative to the generated file, so the pair can live anywhere.
/// rustc tracks both files in its dep-info, so the including crate is rebuilt
/// whenever they change.
fn embed_artifact(pkg: &str, constant: &str, artifact: &Path, output: &Path) -> Result<()> {
    let object = output.with_extension("o");
    let object_name = object
        .file_name()
        .and_then(|n| n.to_str())
        .with_context(|| format!("invalid output path `{}`", output.display()))?;

    if let Some(dir) = output.parent().filter(|d| !d.as_os_str().is_empty()) {
        fs::create_dir_all(dir).with_context(|| format!("failed to create `{}`", dir.display()))?;
    }

    let bytes =
        fs::read(artifact).with_context(|| format!("failed to read `{}`", artifact.display()))?;
    let sha256 = hex::encode(Sha256::digest(&bytes));

    fs::write(&object, &bytes)
        .with_context(|| format!("failed to write `{}`", object.display()))?;
    println!(
        "  copied  `{}` → `{}`",
        artifact.display(),
        object.display()
    );

    let mut generated = String::new();
    write!(
        generated,
        "// @generated by `cargo xtask build-ebpf -p {pkg}`. Do not edit.\n\
         pub static {constant}: crate::EbpfObject = crate::EbpfObject {{\n    \
         name: {pkg:?},\n    \
         bytes: aya::include_bytes_aligned!({object_name:?}),\n    \
         sha256: {sha256:?},\n\
         }};\n"
    )?;

    fs::write(output, generated)
        .with_context(|| format!("failed to write `{}`", output.display()))?;
    println!("  wrote   `{}`", output.display());

    Ok(())
}
