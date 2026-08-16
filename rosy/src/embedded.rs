//! # Embedded Runtime Scaffolding
//!
//! Writes the vendored `rosy_lib` runtime and project scaffolding (Cargo.toml,
//! main.rs template) into the generated output directory. This makes every
//! transpiled project self-contained — it compiles without depending on the
//! rosy transpiler installation.
//!
//! The `rosy_lib` source files are embedded into the transpiler binary at
//! compile time via `include_str!()` (see `build.rs`) and extracted into
//! generated projects as a self-contained vendored crate.

use anyhow::{Context, Result};
use std::path::Path;

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

// Include the auto-generated embedded rosy_lib files
include!(concat!(env!("OUT_DIR"), "/embedded_rosy_lib.rs"));

/// Embedded main.rs template for generated projects
const MAIN_RS_TEMPLATE: &str = include_str!("../assets/output_template/main.rs");

/// Writes the vendored rosy_lib to the output directory
fn write_vendored_lib(output_dir: &Path) -> Result<()> {
    let lib_dir = output_dir.join("vendored/rosy_lib");

    for embedded_file in ROSY_LIB_FILES {
        let target_path = lib_dir.join("src").join(embedded_file.path);

        if let Some(parent) = target_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }

        write_if_changed(&target_path, embedded_file.content)
            .with_context(|| format!("Failed to write file: {}", target_path.display()))?;
    }

    // Write Cargo.toml for rosy_lib with MPI and SIMD as optional features
    let lib_cargo_toml = r#"[package]
name = "rosy_lib"
version = "0.1.0"
edition = "2024"

[features]
mpi = ["dep:mpi", "dep:bincode"]
nightly-simd = []

[dependencies]
anyhow = "1.0"
mpi = { version = "0.8", optional = true }
bincode = { version = "2.0", optional = true }
num-complex = "0.4"
rustc-hash = "2"
rand = "0.9"
libm = "0.2"
memory-stats = "1"
libc = "0.2"
"#;

    write_if_changed(lib_dir.join("Cargo.toml"), lib_cargo_toml)
        .context("Failed to write vendored rosy_lib Cargo.toml")?;

    Ok(())
}

/// Generates a Cargo.toml for the output project
fn generate_cargo_toml(uses_mpi: bool, optimized: bool) -> String {
    let mut features = Vec::new();
    if uses_mpi {
        features.push("\"mpi\"");
    }
    if optimized {
        features.push("\"nightly-simd\"");
    }

    let mpi_dep = if features.is_empty() {
        "rosy_lib = { path = \"./vendored/rosy_lib\" }".to_string()
    } else {
        format!(
            "rosy_lib = {{ path = \"./vendored/rosy_lib\", features = [{}] }}",
            features.join(", ")
        )
    };

    let profile_section = if optimized {
        "\n[profile.release]\nopt-level = 3\nlto = \"fat\"\ncodegen-units = 1\npanic = \"abort\"\n"
    } else {
        // codegen-units = 1 is essential: DA hot paths span taylor/ and intrinsics/ modules,
        // and cross-unit inlining requires LTO or single codegen unit.
        "\n[profile.release]\ncodegen-units = 1\n"
    };

    format!(
        "[workspace]\n\n[package]\nname = \"rosy_output\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nanyhow = \"1.0\"\n{mpi_dep}\nnum-complex = \"0.4\"\n{profile_section}"
    )
}

/// Creates the output project structure in the specified directory
pub fn create_output_project(output_dir: &Path, uses_mpi: bool, optimized: bool) -> Result<()> {
    // Create the directory structure
    std::fs::create_dir_all(output_dir.join("src"))
        .context("Failed to create output directory structure")?;

    // Write vendored rosy_lib
    write_vendored_lib(output_dir).context("Failed to write vendored rosy_lib")?;

    // Write Cargo.toml
    write_if_changed(
        output_dir.join("Cargo.toml"),
        generate_cargo_toml(uses_mpi, optimized),
    )
    .context("Failed to write Cargo.toml template")?;

    // Write main.rs template
    std::fs::write(output_dir.join("src/main.rs"), MAIN_RS_TEMPLATE)
        .context("Failed to write main.rs template")?;

    Ok(())
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
