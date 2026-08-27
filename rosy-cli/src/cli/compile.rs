use anyhow::{Context, Result, anyhow};
use rosy_compiler::{ast, embedded, program::Program, resolve, transpile::*};
use std::{
    fs::write,
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};

use super::{BOLD, CYAN, DIM, GREEN, RED, RESET, YELLOW};

pub(crate) fn step(num: usize, total: usize, label: &str) {
    eprint!("{BOLD}{CYAN}[{num}/{total}]{RESET} {label}...");
}
pub(crate) fn step_done(start: Instant) {
    let ms = start.elapsed().as_millis();
    eprintln!(" {GREEN}done{RESET} {DIM}({ms}ms){RESET}");
}
pub(crate) fn step_fail() {
    eprintln!(" {RED}failed{RESET}");
}

pub(crate) fn rosy(
    script_path: &PathBuf,
    output_dir: Option<PathBuf>,
    release: bool,
    optimized: bool,
) -> Result<PathBuf> {
    let filename = script_path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".into());
    let profile_label = if optimized {
        "optimized"
    } else if release {
        "release"
    } else {
        "debug"
    };
    eprintln!("{BOLD}        Rosy{RESET} v{}", env!("CARGO_PKG_VERSION"));
    eprintln!("{BOLD}  Transpiling{RESET} {filename} ({profile_label})");

    step(1, 6, "Reading");
    let t = Instant::now();
    let raw_script = std::fs::read_to_string(script_path).with_context(|| {
        format!(
            "Failed to read script file from `{}`!",
            script_path.display()
        )
    })?;
    step_done(t);

    compile_source(
        &raw_script,
        Some(script_path.as_path()),
        output_dir.unwrap_or_else(|| PathBuf::from(".rosy_output")),
        release,
        optimized,
        false,
    )
}

pub(crate) fn compile_source(
    raw_script: &str,
    script_path: Option<&Path>,
    rosy_output_path: PathBuf,
    release: bool,
    optimized: bool,
    quiet: bool,
) -> Result<PathBuf> {
    let total_start = Instant::now();

    if !quiet {
        step(2, 6, "Parsing");
    }
    let t = Instant::now();
    rosy_compiler::syntax_config::apply_from_path(script_path);
    let program = ast::parse_source(raw_script)
        .context("Couldn't parse!")?
        .next()
        .context("Expected a program")?;
    if !quiet {
        step_done(t);
        step(3, 6, "Building AST");
    }

    let t = Instant::now();
    let mut ast = Program::from_rule_with_includes(
        program,
        script_path,
        &mut rosy_compiler::program::IncludeTracker::default(),
    )
    .context("Failed to build AST!")?
    .context("Expected a program")?;
    if !quiet {
        step_done(t);
        step(4, 6, "Resolving types");
    }

    let t = Instant::now();
    let (_resolver, warnings) =
        resolve::TypeResolver::resolve(&mut ast).context("Failed to resolve types!")?;
    if !quiet {
        step_done(t);
        for w in &warnings {
            eprintln!("{BOLD}{YELLOW}    warning{RESET}: {}", w.message);
        }
        step(5, 6, "Generating Rust code");
    }

    let t = Instant::now();
    let TranspilationOutput { serialization, .. } = ast
        .transpile(&mut TranspilationInputContext::default())
        .map_err(|vec_errs| {
            if !quiet {
                step_fail();
            }
            let mut combined = String::new();
            for (outer_ind, err) in vec_errs.iter().enumerate() {
                let mut body = String::new();
                for (ind, ctx) in err.chain().enumerate() {
                    body.push_str(&format!("  {}. {}\n", ind + 1, ctx));
                }
                combined.push_str(&format!(
                    "\n#{}: {}\nContext:\n{}",
                    outer_ind + 1,
                    err.root_cause(),
                    body
                ));
            }
            anyhow!(
                "Failed to transpile with the following errors:\n{}",
                combined
            )
        })?;

    let uses_mpi = serialization.contains("rosy_mpi_context");

    let local_lib = embedded::create_output_project(&rosy_output_path, uses_mpi, optimized)
        .context("Failed to create output project structure")?;
    if let Some(ref local) = local_lib {
        if !quiet {
            eprintln!(
                "{BOLD}{YELLOW}    warning{RESET}: using local rosy-lib ({})",
                local.display()
            );
        }
    }

    let new_contents = embedded::inject_code(&serialization, uses_mpi)
        .context("Failed to inject transpiled code into template")?;

    write(rosy_output_path.join("src/main.rs"), &new_contents)
        .context("Failed to write Rust output file!")?;
    if !quiet {
        step_done(t);
        eprintln!("{BOLD}{CYAN}[6/6]{RESET} Compiling generated Rust code...");
    }

    let mut cargo_args = vec!["build", "--bin", "rosy_output"];
    if !quiet {
        cargo_args.push("--color");
        cargo_args.push("always");
    }
    if release {
        cargo_args.push("--release");
    }

    let mut cmd = Command::new("cargo");
    cmd.args(&cargo_args)
        .current_dir(&rosy_output_path)
        .stdin(std::process::Stdio::null());
    if quiet {
        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
    }
    let output = cmd
        .output()
        .context("Failed to spawn cargo build process")?;
    if !output.status.success() {
        if quiet {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("generated code failed to compile\n{stderr}");
        }
        eprintln!();
        eprintln!("{BOLD}{RED}  error:{RESET} The generated Rust code failed to compile.");
        eprintln!();
        eprintln!("  This is a bug in the Rosy transpiler, not in your code.");
        eprintln!("  Please report it at: {BOLD}https://github.com/rosy-team/rosy/issues{RESET}");
        eprintln!("  Include your {BOLD}source{RESET} files and the error output above.");
        anyhow::bail!(
            "Internal transpiler error: generated code failed to compile (exit code {:?})",
            output.status.code()
        );
    }

    let build_profile = if release { "release" } else { "debug" };
    let binary_name = if cfg!(windows) {
        "rosy_output.exe"
    } else {
        "rosy_output"
    };
    let binary_path = rosy_output_path.join(format!("target/{}/{}", build_profile, binary_name));

    if !quiet {
        let total_ms = total_start.elapsed().as_millis();
        eprintln!(
            "{BOLD}{GREEN}    Finished{RESET} in {DIM}{:.2}s{RESET}",
            total_ms as f64 / 1000.0
        );
    }

    Ok(binary_path)
}
