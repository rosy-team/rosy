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

fn local_rosy_lib() -> Option<PathBuf> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../rosy-lib");
    p.is_dir().then(|| p.canonicalize().ok()).flatten()
}

/// Generates a Cargo.toml for the output project
fn generate_cargo_toml(optimized: bool, rosy_lib_dep: &str) -> String {

    let profile_section = if optimized {
        "\n[profile.release]\nopt-level = 3\nlto = \"fat\"\ncodegen-units = 1\npanic = \"abort\"\n"
    } else {
        // codegen-units = 1 is essential: DA hot paths span taylor/ and intrinsics/ modules,
        // and cross-unit inlining requires LTO or single codegen unit.
        "\n[profile.release]\ncodegen-units = 1\n"
    };

    format!(
        "[workspace]\n\n[package]\nname = \"rosy_output\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nanyhow = \"1.0\"\n{rosy_lib_dep}\nnum-complex = \"0.4\"\n{profile_section}"
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
        generate_cargo_toml(optimized, &rosy_lib_dep),
    )
    .context("Failed to write Cargo.toml template")?;

    // Write main.rs template
    std::fs::write(output_dir.join("src/main.rs"), MAIN_RS_TEMPLATE)
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
