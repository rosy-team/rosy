//! # Embedded Runtime Scaffolding
//!
//! Writes project scaffolding (Cargo.toml, main.rs template) into the
//! generated output directory. `rosy-lib` comes from the local checkout
//! when present, otherwise crates.io.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

fn write_if_changed(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> std::io::Result<()> {
    let path = path.as_ref();
    let contents = contents.as_ref();
    if path
        .exists()
        .then(|| std::fs::read(path).ok())
        .flatten()
        .as_deref()
        == Some(contents)
    {
        return Ok(());
    }
    std::fs::write(path, contents)
}

/// Embedded main.rs template for generated projects
const MAIN_RS_TEMPLATE: &str = include_str!("../../assets/output_template/main.rs");

/// Strip Windows `\\?\` / `\\?\UNC\` prefixes from `canonicalize()` so the
/// path is usable in Cargo.toml and human-readable in diagnostics.
fn strip_verbatim_prefix(path: PathBuf) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{rest}"))
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        path
    }
}

fn local_rosy_lib() -> Option<PathBuf> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("rosy-lib");
    p.is_dir()
        .then(|| p.canonicalize().ok())
        .flatten()
        .map(strip_verbatim_prefix)
}

/// Generates a Cargo.toml for the output project
fn generate_cargo_toml(optimized: bool, rosy_lib_dep: &str, kernels: bool) -> String {
    let build_deps = if kernels { "\n[build-dependencies]\ncc = \"1\"\n" } else { "" };
    let profile_section = if optimized {
        "\n[profile.release]\nopt-level = 3\nlto = \"fat\"\ncodegen-units = 1\npanic = \"abort\"\n"
    } else {
        // codegen-units = 1 is essential: DA hot paths span taylor/ and intrinsics/ modules,
        // and cross-unit inlining requires LTO or single codegen unit.
        "\n[profile.release]\ncodegen-units = 1\n"
    };

    format!(
        "[workspace]\n\n[package]\nname = \"rosy-output\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nanyhow = \"1.0\"\n{rosy_lib_dep}\nnum-complex = \"0.4\"\n{build_deps}{profile_section}"
    )
}

/// Creates the output project structure in the specified directory.
///
/// Returns `Some(local rosy-lib path)` when the workspace checkout is used
/// instead of the vendored copy.
pub fn create_output_project(
    output_dir: &Path,
    uses_mpi: bool,
    optimized: bool,
    kernels: &[PathBuf],
) -> Result<Option<PathBuf>> {
    // Create the directory structure
    std::fs::create_dir_all(output_dir.join("src"))
        .context("Failed to create output directory structure")?;

    let mut features = Vec::new();
    if uses_mpi {
        features.push("\"mpi\"");
    }
    if optimized {
        features.push("\"nightly-simd\"");
    }
    let features_toml = if features.is_empty() {
        String::new()
    } else {
        format!(", features = [{}]", features.join(", "))
    };

    let local = local_rosy_lib();
    let rosy_lib_dep = if let Some(ref local) = local {
        let path = local.to_string_lossy().replace('\\', "/");
        format!("rosy-lib = {{ path = \"{path}\"{features_toml} }}")
    } else {
        format!(
            "rosy-lib = {{ version = \"{}\"{features_toml} }}",
            env!("CARGO_PKG_VERSION")
        )
    };

    // Write Cargo.toml
    write_if_changed(
        output_dir.join("Cargo.toml"),
        generate_cargo_toml(optimized, &rosy_lib_dep, !kernels.is_empty()),
    )
    .context("Failed to write Cargo.toml template")?;

    // C kernels the program supplies (`KERNEL`) are compiled by a build script.
    // ROSY_KERNEL_CFLAGS and ROSY_KERNEL_LIBS, read when the program is
    // built, add compiler flags and libraries to link.
    let build_rs = output_dir.join("build.rs");
    if kernels.is_empty() {
        if build_rs.exists() {
            std::fs::remove_file(&build_rs).context("Failed to remove a stale build.rs")?;
        }
    } else {
        let mut script = String::from("fn main() {\n    let mut build = cc::Build::new();\n    build.opt_level(3);\n");
        for kernel in kernels {
            let path = kernel.to_string_lossy().replace('\\', "/");
            script.push_str(&format!("    build.file({path:?});\n    println!(\"cargo:rerun-if-changed={{}}\", {path:?});\n"));
            // Headers beside the kernel may change too.
            if let Some(dir) = kernel.parent() {
                let dir = dir.to_string_lossy().replace('\\', "/");
                script.push_str(&format!("    println!(\"cargo:rerun-if-changed={{}}\", {dir:?});\n"));
            }
        }
        script.push_str(concat!(
            "    println!(\"cargo:rerun-if-env-changed=ROSY_KERNEL_CFLAGS\");\n",
            "    println!(\"cargo:rerun-if-env-changed=ROSY_KERNEL_LIBS\");\n",
            "    for flag in std::env::var(\"ROSY_KERNEL_CFLAGS\").unwrap_or_default().split_whitespace() {\n        build.flag(flag);\n    }\n",
            "    build.compile(\"rosy_kernels\");\n",
            "    for lib in std::env::var(\"ROSY_KERNEL_LIBS\").unwrap_or_default().split_whitespace() {\n        println!(\"cargo:rustc-link-lib={lib}\");\n    }\n",
            "}\n",
        ));
        write_if_changed(&build_rs, script).context("Failed to write build.rs")?;
    }

    // Write main.rs template
    std::fs::write(output_dir.join("src").join("main.rs"), MAIN_RS_TEMPLATE)
        .context("Failed to write main.rs template")?;

    Ok(local)
}

/// Injects the transpiled code into the main.rs template.
///
/// When `uses_mpi` is false, the MPI initialization block (between
/// `// <MPI_START>` and `// <MPI_END>`) is stripped from the output.
pub fn inject_code(transpiled_code: &str, uses_mpi: bool) -> Result<String> {
    let mut template = MAIN_RS_TEMPLATE.to_string();

    // Strip MPI initialization block when not needed
    if !uses_mpi {
        let mpi_parts: Vec<&str> = template.split("// <MPI_START>").collect();
        anyhow::ensure!(
            mpi_parts.len() == 2,
            "Expected exactly one '// <MPI_START>' in main.rs template!"
        );
        let before_mpi = mpi_parts[0];
        let after_mpi_parts: Vec<&str> = mpi_parts[1].split("// <MPI_END>").collect();
        anyhow::ensure!(
            after_mpi_parts.len() == 2,
            "Expected exactly one '// <MPI_END>' in main.rs template!"
        );
        let after_mpi = after_mpi_parts[1];
        template = format!("{}{}", before_mpi, after_mpi);
    }

    // Split by injection markers
    let parts: Vec<&str> = template.split("// <INJECT_START>").collect();
    anyhow::ensure!(
        parts.len() == 2,
        "Expected exactly one '// <INJECT_START>' in main.rs template!"
    );

    let before_inject = parts[0];
    let parts: Vec<&str> = parts[1].split("// <INJECT_END>").collect();
    anyhow::ensure!(
        parts.len() == 2,
        "Expected exactly one '// <INJECT_END>' in main.rs template!"
    );

    let after_inject = parts[1];

    // Format the transpiled code with proper indentation
    let indented_code = transpiled_code
        .lines()
        .map(|line| format!("\t{}", line))
        .collect::<Vec<String>>()
        .join("\n");

    Ok(format!(
        "{}// <INJECT_START>\n{}\n\t// <INJECT_END>{}",
        before_inject, indented_code, after_inject
    ))
}
