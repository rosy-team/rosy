mod update_check;

use anyhow::{Context, Result, anyhow, ensure};
use clap::{Parser as ClapParser, Subcommand};
use pest::Parser;
use rosy::{ast, embedded, program::Program, resolve, syntax_config, transpile::*};
use std::{
    fs,
    fs::write,
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};

// ANSI color helpers (stderr only)
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const GREEN: &str = "\x1b[32m";
const CYAN: &str = "\x1b[36m";
const YELLOW: &str = "\x1b[33m";
const RED: &str = "\x1b[31m";
const RESET: &str = "\x1b[0m";

fn step(num: usize, total: usize, label: &str) {
    eprint!("{BOLD}{CYAN}[{num}/{total}]{RESET} {label}...");
}
fn step_done(start: Instant) {
    let ms = start.elapsed().as_millis();
    eprintln!(" {GREEN}done{RESET} {DIM}({ms}ms){RESET}");
}
fn step_fail() {
    eprintln!(" {RED}failed{RESET}");
}

/// Rosy Transpiler - Converts Rosy source code to executable Rust programs
#[derive(ClapParser)]
#[command(name = "rosy")]
#[command(version)]
#[command(about = "Rosy Transpiler for beam physics calculations", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a Rosy script directly without copying binary to PWD
    Run {
        /// Path to the Rosy source file
        source: PathBuf,

        /// Output directory for build artifacts (default: .rosy_output)
        #[arg(short = 'd', long)]
        output_dir: Option<PathBuf>,

        /// Build in release mode with optimizations
        #[arg(short, long)]
        release: bool,

        /// Aggressive optimizations: LTO, single codegen unit, panic=abort, SIMD DA (slower builds, faster binaries; requires nightly Rust)
        #[arg(long)]
        optimized: bool,

        /// Enforce COSY INFINITY syntax: memory sizes are required in VARIABLE declarations
        #[arg(long)]
        cosy_syntax: bool,
    },

    /// Run language feature tests (transpile, compile, execute each construct)
    Test {
        /// Only run tests whose name contains this string
        #[arg(short, long)]
        filter: Option<String>,

        /// Run tests in release mode
        #[arg(short, long)]
        release: bool,

        /// Rewrite `======= expect` in fixtures from this run
        #[arg(long)]
        bless: bool,
    },

    /// Build a Rosy script and place the binary in PWD
    Build {
        /// Path to the Rosy source file
        source: PathBuf,

        /// Output binary name (default: source filename without extension)
        #[arg(short, long)]
        output: Option<String>,

        /// Output directory for build artifacts (default: .rosy_output)
        #[arg(short = 'd', long)]
        output_dir: Option<PathBuf>,

        /// Build in release mode with optimizations
        #[arg(short, long)]
        release: bool,

        /// Aggressive optimizations: LTO, single codegen unit, panic=abort, SIMD DA (slower builds, faster binaries; requires nightly Rust)
        #[arg(long)]
        optimized: bool,

        /// Enforce COSY INFINITY syntax: memory sizes are required in VARIABLE declarations
        #[arg(long)]
        cosy_syntax: bool,
    },

    /// Start the Language Server Protocol (LSP) server on stdin/stdout
    Lsp {
        /// Accepted for compatibility with editors that inject --stdio (e.g. VS Code)
        #[arg(long, hide = true)]
        stdio: bool,
    },

    /// Install editor extensions for Rosy language support
    Setup {
        /// Which editor to install for
        #[command(subcommand)]
        editor: EditorTarget,
    },
}

#[derive(Subcommand)]
enum EditorTarget {
    /// Install the VS Code extension (syntax highlighting + LSP)
    Vscode,
    /// Install the Zed extension (language config + LSP setup)
    Zed,
}

fn rosy(
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

fn compile_source(
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
    let program = ast::CosyParser::parse(ast::Rule::program, raw_script)
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
        &mut rosy::program::IncludeTracker::default(),
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

    embedded::create_output_project(&rosy_output_path, uses_mpi, optimized)
        .context("Failed to create output project structure")?;

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
        eprintln!("  Include your {BOLD}.rosy{RESET} file and the error output above.");
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

// ─── Construct Test Runner (`rosy test`) ────────────────────────────────────

struct CaseFile {
    source: String,
    expect: Option<String>,
    fox: Option<String>,
}

fn parse_case(text: &str) -> CaseFile {
    let mut source = String::new();
    let mut expect = None;
    let mut fox = None;
    let mut dest = 0u8; // 0 source, 1 expect, 2 fox, 3 other
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("======= ") {
            dest = match rest.trim() {
                "expect" => {
                    expect = Some(String::new());
                    1
                }
                "fox" => {
                    fox = Some(String::new());
                    2
                }
                _ => 3,
            };
            continue;
        }
        let buf = match dest {
            1 => expect.as_mut().unwrap(),
            2 => fox.as_mut().unwrap(),
            3 => continue,
            _ => &mut source,
        };
        if !buf.is_empty() {
            buf.push('\n');
        }
        buf.push_str(line);
    }
    CaseFile {
        source,
        expect,
        fox,
    }
}

fn norm_out(s: &str) -> String {
    s.lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn write_blessed(path: &Path, case: &CaseFile, stdout: &str) -> Result<()> {
    let mut out = case.source.trim_end().to_string();
    out.push_str("\n\n======= expect\n");
    out.push_str(stdout.trim_end());
    out.push('\n');
    if let Some(fox) = &case.fox {
        out.push_str("\n======= fox\n");
        out.push_str(fox.trim_end());
        out.push('\n');
    }
    fs::write(path, out)?;
    Ok(())
}

fn discover_case_files(base: &Path) -> Vec<(String, PathBuf)> {
    let mut results = Vec::new();
    discover_case_files_recursive(base, base, &mut results);
    results.sort_by(|a, b| a.0.cmp(&b.0));
    results
}

fn discover_case_files_recursive(root: &Path, dir: &Path, results: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.path());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            discover_case_files_recursive(root, &path, results);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rosy") {
            let name = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .with_extension("")
                .to_string_lossy()
                .to_string();
            results.push((name, path));
        }
    }
}

/// Result of a single construct test.
#[derive(Debug)]
struct TestResult {
    label: String,
    ok: bool,
    elapsed_secs: f64,
    failure_msg: Option<String>,
}

fn run_single_test(
    name: &str,
    case_path: &Path,
    build_dir: &Path,
    release: bool,
    bless: bool,
) -> TestResult {
    let t = Instant::now();
    let raw = match fs::read_to_string(case_path) {
        Ok(s) => s,
        Err(e) => {
            return TestResult {
                label: name.to_string(),
                ok: false,
                elapsed_secs: t.elapsed().as_secs_f64(),
                failure_msg: Some(format!("read failed: {e}")),
            };
        }
    };
    let case = parse_case(&raw);

    let binary = match compile_source(
        &case.source,
        Some(case_path),
        build_dir.to_path_buf(),
        release,
        false,
        true,
    ) {
        Ok(p) => p,
        Err(e) => {
            return TestResult {
                label: name.to_string(),
                ok: false,
                elapsed_secs: t.elapsed().as_secs_f64(),
                failure_msg: Some(format!("{e:#}")),
            };
        }
    };

    let output = match Command::new(&binary)
        .current_dir(case_path.parent().unwrap_or(case_path))
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            return TestResult {
                label: name.to_string(),
                ok: false,
                elapsed_secs: t.elapsed().as_secs_f64(),
                failure_msg: Some(format!("failed to spawn: {e}")),
            };
        }
    };
    if !output.status.success() {
        return TestResult {
            label: name.to_string(),
            ok: false,
            elapsed_secs: t.elapsed().as_secs_f64(),
            failure_msg: Some(format!(
                "runtime failed\n{}",
                String::from_utf8_lossy(&output.stderr)
            )),
        };
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    if bless {
        if let Err(e) = write_blessed(case_path, &case, &stdout) {
            return TestResult {
                label: name.to_string(),
                ok: false,
                elapsed_secs: t.elapsed().as_secs_f64(),
                failure_msg: Some(format!("bless failed: {e}")),
            };
        }
        return TestResult {
            label: name.to_string(),
            ok: true,
            elapsed_secs: t.elapsed().as_secs_f64(),
            failure_msg: None,
        };
    }

    let Some(expect) = case.expect else {
        return TestResult {
            label: name.to_string(),
            ok: false,
            elapsed_secs: t.elapsed().as_secs_f64(),
            failure_msg: Some("missing `======= expect` (rosy test --bless)".into()),
        };
    };
    if norm_out(&stdout) != norm_out(&expect) {
        return TestResult {
            label: name.to_string(),
            ok: false,
            elapsed_secs: t.elapsed().as_secs_f64(),
            failure_msg: Some(format!(
                "stdout mismatch\n--- expected ---\n{}\n--- got ---\n{}",
                expect.trim_end(),
                stdout.trim_end()
            )),
        };
    }

    TestResult {
        label: name.to_string(),
        ok: true,
        elapsed_secs: t.elapsed().as_secs_f64(),
        failure_msg: None,
    }
}

fn run_construct_tests(filter: Option<&str>, release: bool, bless: bool) -> Result<()> {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cases_dir = crate_root.join("tests/constructs");
    let mut all_tests = discover_case_files(&cases_dir);

    if let Some(f) = filter {
        all_tests.retain(|(name, _)| name.contains(f));
    }

    let total = all_tests.len();
    if total == 0 {
        eprintln!(
            "No tests found{}",
            filter
                .map(|f| format!(" matching '{f}'"))
                .unwrap_or_default()
        );
        return Ok(());
    }

    eprintln!(
        "{BOLD}        Rosy{RESET} v{} — testing {total} construct{}",
        env!("CARGO_PKG_VERSION"),
        if total == 1 { "" } else { "s" }
    );
    if release {
        eprintln!("        Mode: release");
    }
    if bless {
        eprintln!("        Bless: rewriting expect sections");
    }

    let build_dir = std::env::temp_dir().join(format!("rosy_test_{}", std::process::id()));
    fs::create_dir_all(&build_dir).context("Failed to create build directory")?;
    eprintln!("  Build dir: {}\n", build_dir.display());

    let total_start = Instant::now();
    let mut results = Vec::with_capacity(total);

    for (i, (name, path)) in all_tests.iter().enumerate() {
        let result = run_single_test(name, path, &build_dir, release, bless);
        let n = i + 1;
        if result.ok {
            eprintln!(
                "{DIM}[{n:>3}/{total}]{RESET} {}... {GREEN}ok{RESET} {DIM}({:.1}s){RESET}",
                result.label, result.elapsed_secs
            );
        } else {
            eprintln!(
                "{DIM}[{n:>3}/{total}]{RESET} {}... {RED}FAIL{RESET} {DIM}({:.1}s){RESET}",
                result.label, result.elapsed_secs
            );
        }
        results.push(result);
    }

    let _ = fs::remove_dir_all(&build_dir);

    let passed = results.iter().filter(|r| r.ok).count();
    let failed = results.iter().filter(|r| !r.ok).count();
    let total_secs = total_start.elapsed().as_secs_f64();

    eprintln!();
    let failures: Vec<&TestResult> = results.iter().filter(|r| !r.ok).collect();
    if !failures.is_empty() {
        eprintln!("{BOLD}{RED}failures:{RESET}\n");
        for f in &failures {
            eprintln!(
                "  {}: {}\n",
                f.label,
                f.failure_msg.as_deref().unwrap_or("unknown")
            );
        }
    }

    eprintln!(
        "{BOLD}test result:{RESET} {} passed, {} failed ({:.1}s)",
        passed, failed, total_secs
    );

    if failed > 0 {
        Err(anyhow!("{} test(s) failed", failed))
    } else {
        Ok(())
    }
}

// ─── Editor Extension Installer ────────────────────────────────────────────

// VS Code extension files embedded at compile time
// VS Code extension files — package.json, extension.js, and tmLanguage are
// static (embedded from editors/vscode/). The language config is generated
// from the grammar at build time so folding/indent keywords stay in sync.
const VSCODE_PACKAGE_JSON: &str = include_str!("../assets/editors/vscode/package.json");
const VSCODE_LANG_CONFIG: &str = include_str!(concat!(
    env!("OUT_DIR"),
    "/vscode_language_configuration.json"
));
const VSCODE_EXTENSION_JS: &str = include_str!("../assets/editors/vscode/extension.js");
const VSCODE_TM_GRAMMAR: &str =
    include_str!("../assets/editors/vscode/syntaxes/rosy.tmLanguage.json");

// Zed extension files — embedded at build time.
// Unlike VS Code, Zed extensions need a WASM component, so we write the
// full extension source directory and the user installs it as a dev extension.
const ZED_EXTENSION_TOML: &str = include_str!("../assets/editors/zed/extension.toml");
const ZED_CARGO_TOML: &str = include_str!("../assets/editors/zed/Cargo.toml");
const ZED_LIB_RS: &str = include_str!("../assets/editors/zed/src/lib.rs");
const ZED_CONFIG_TOML: &str = include_str!("../assets/editors/zed/languages/rosy/config.toml");
const ZED_HIGHLIGHTS_SCM: &str = include_str!(concat!(env!("OUT_DIR"), "/highlights.scm"));

fn install_editor_extension(editor: &EditorTarget) -> Result<()> {
    match editor {
        EditorTarget::Vscode => install_vscode_extension(),
        EditorTarget::Zed => install_zed_extension(),
    }
}

fn install_vscode_extension() -> Result<()> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .context("Could not determine home directory (neither HOME nor USERPROFILE is set)")?;

    let extensions_dir = PathBuf::from(&home).join(".vscode/extensions");

    // Clean up old extension directories from before the naming fix
    for old_name in ["rosy-language-support", "rosy-team.rosy-language-support"] {
        let old_ext_dir = extensions_dir.join(old_name);
        if old_ext_dir.exists() {
            eprintln!(
                "{DIM}  Removing old extension at {}{RESET}",
                old_ext_dir.display()
            );
            let _ = fs::remove_dir_all(&old_ext_dir);
        }
    }

    // Also clean up any previous versioned installs (rosy-team.rosy-language-support-*)
    if let Ok(entries) = fs::read_dir(&extensions_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("rosy-team.rosy-language-support-") {
                eprintln!(
                    "{DIM}  Removing old extension at {}{RESET}",
                    entry.path().display()
                );
                let _ = fs::remove_dir_all(entry.path());
            }
        }
    }

    let ext_version = env!("CARGO_PKG_VERSION");
    let ext_dir = extensions_dir.join(format!("rosy-team.rosy-language-support-{ext_version}"));
    let syntaxes_dir = ext_dir.join("syntaxes");

    let action = if ext_dir.exists() {
        "Updating"
    } else {
        "Installing"
    };
    eprintln!("{BOLD}  {action}{RESET} VS Code extension");
    eprintln!("         to: {}", ext_dir.display());

    fs::create_dir_all(&syntaxes_dir).context("Failed to create extension directory")?;

    // Inject the transpiler version into package.json so it matches the folder name
    let package_json = VSCODE_PACKAGE_JSON.replace(
        "\"version\": \"0.0.0-injected\"",
        &format!("\"version\": \"{}\"", ext_version),
    );
    write(ext_dir.join("package.json"), package_json)?;
    write(
        ext_dir.join("language-configuration.json"),
        VSCODE_LANG_CONFIG,
    )?;
    write(ext_dir.join("extension.js"), VSCODE_EXTENSION_JS)?;
    write(syntaxes_dir.join("rosy.tmLanguage.json"), VSCODE_TM_GRAMMAR)?;

    // Register the extension in VS Code's extensions.json registry
    let registry_path = extensions_dir.join("extensions.json");
    let mut registry: Vec<serde_json::Value> = if registry_path.exists() {
        let contents =
            fs::read_to_string(&registry_path).context("Failed to read extensions.json")?;
        serde_json::from_str(&contents).unwrap_or_default()
    } else {
        Vec::new()
    };

    // Remove any existing rosy entries
    registry.retain(|entry| {
        entry
            .get("identifier")
            .and_then(|id| id.get("id"))
            .and_then(|id| id.as_str())
            != Some("rosy-team.rosy-language-support")
    });

    let relative_location = format!("rosy-team.rosy-language-support-{ext_version}");
    registry.push(serde_json::json!({
        "identifier": { "id": "rosy-team.rosy-language-support" },
        "version": ext_version,
        "location": {
            "$mid": 1,
            "path": ext_dir.to_string_lossy(),
            "scheme": "file"
        },
        "relativeLocation": relative_location,
        "metadata": {
            "installedTimestamp": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            "source": "rosy-setup"
        }
    }));

    let registry_json =
        serde_json::to_string_pretty(&registry).context("Failed to serialize extensions.json")?;
    write(&registry_path, registry_json)?;

    let done_verb = if action == "Updating" {
        "Updated"
    } else {
        "Installed"
    };
    eprintln!("{BOLD}{GREEN}    {done_verb}{RESET} Rosy Language Support for VS Code");
    eprintln!();
    eprintln!("  Reload VS Code to activate. Open any {BOLD}.rosy{RESET} file to see");
    eprintln!("  syntax highlighting, diagnostics, and type hints.");
    eprintln!();
    eprintln!("  {DIM}Make sure `rosy` is in your PATH so the LSP server can start.{RESET}");

    Ok(())
}

fn install_zed_extension() -> Result<()> {
    // Write the full extension source directory. Zed compiles the WASM
    // component when the user installs it as a dev extension.
    let ext_dir = if cfg!(target_os = "windows") {
        let appdata = std::env::var("LOCALAPPDATA")
            .context("Could not determine data directory (LOCALAPPDATA is not set)")?;
        PathBuf::from(appdata).join("rosy/zed-extension")
    } else {
        let home = std::env::var("HOME")
            .context("Could not determine home directory (HOME is not set)")?;
        PathBuf::from(home).join(".local/share/rosy/zed-extension")
    };
    let src_dir = ext_dir.join("src");
    let languages_dir = ext_dir.join("languages/rosy");

    let action = if ext_dir.exists() {
        "Updating"
    } else {
        "Writing"
    };
    eprintln!("{BOLD}  {action}{RESET} Zed extension source");
    eprintln!("         to: {}", ext_dir.display());

    fs::create_dir_all(&src_dir).context("Failed to create extension source directory")?;
    fs::create_dir_all(&languages_dir).context("Failed to create languages directory")?;

    write(ext_dir.join("extension.toml"), ZED_EXTENSION_TOML)?;
    write(ext_dir.join("Cargo.toml"), ZED_CARGO_TOML)?;
    write(src_dir.join("lib.rs"), ZED_LIB_RS)?;
    write(languages_dir.join("config.toml"), ZED_CONFIG_TOML)?;
    write(languages_dir.join("highlights.scm"), ZED_HIGHLIGHTS_SCM)?;

    let done_verb = if action == "Updating" {
        "Updated"
    } else {
        "Wrote"
    };
    eprintln!("{BOLD}{GREEN}    {done_verb}{RESET} Rosy extension for Zed");
    eprintln!();
    eprintln!("  {BOLD}Prerequisites:{RESET}");
    eprintln!("    Zed compiles extensions to WASM using {BOLD}rustup{RESET}.");
    eprintln!("    If you don't have rustup ({DIM}e.g. NixOS, distro-packaged Rust{RESET}):");
    eprintln!("      {DIM}curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh{RESET}");
    eprintln!("    Then set up the toolchain:");
    eprintln!("      {DIM}rustup default nightly{RESET}");
    eprintln!("      {DIM}rustup target add wasm32-wasip1{RESET}");
    eprintln!("      {DIM}rustup component add rust-src{RESET}");
    eprintln!();
    eprintln!("  {BOLD}To install:{RESET}");
    eprintln!("    1. Open Zed");
    eprintln!("    2. Open the command palette ({DIM}Cmd+Shift+P / Ctrl+Shift+P{RESET})");
    eprintln!("    3. Run {BOLD}zed: install dev extension{RESET}");
    eprintln!("    4. Select: {DIM}{}{RESET}", ext_dir.display());
    eprintln!();
    eprintln!("  Zed will compile the extension and activate it. Open any");
    eprintln!("  {BOLD}.rosy{RESET} file to see diagnostics, completions, and type hints.");
    eprintln!();
    eprintln!("  To enable inlay type hints:");
    eprintln!("    {DIM}Settings → Open Settings → Editor → Inlay Hints → Enabled → On{RESET}");
    eprintln!();
    eprintln!("  {DIM}Make sure `rosy` is in your PATH so the LSP server can start.{RESET}");

    Ok(())
}

fn main() -> Result<()> {
    // Kick off a background version check (non-blocking)
    let update_handle = update_check::spawn_update_check();

    let cli = Cli::parse();

    // Handle LSP command — launch language server on stdin/stdout
    if matches!(&cli.command, Commands::Lsp { .. }) {
        update_handle.finish();
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(rosy::lsp::run());
        return Ok(());
    }

    // Handle Setup command — install editor extensions
    if let Commands::Setup { editor } = &cli.command {
        update_handle.finish();
        return install_editor_extension(editor);
    }

    // Handle Test command separately (no transpilation pipeline)
    if let Commands::Test {
        filter,
        release,
        bless,
    } = &cli.command
    {
        update_handle.finish();
        return run_construct_tests(filter.as_deref(), *release, *bless);
    }

    // Extract common fields and transpile
    let (source, output_dir, release, optimized, cosy_syntax, output_name) = match &cli.command {
        Commands::Run {
            source,
            output_dir,
            release,
            optimized,
            cosy_syntax,
        } => (
            source.clone(),
            output_dir.clone(),
            *release || *optimized,
            *optimized,
            *cosy_syntax,
            None,
        ),
        Commands::Build {
            source,
            output,
            output_dir,
            release,
            optimized,
            cosy_syntax,
        } => {
            let mut name = output.clone().unwrap_or_else(|| {
                source
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("rosy_output")
                    .to_string()
            });
            if cfg!(windows) && !name.ends_with(".exe") {
                name.push_str(".exe");
            }
            (
                source.clone(),
                output_dir.clone(),
                *release || *optimized,
                *optimized,
                *cosy_syntax,
                Some(name),
            )
        }
        Commands::Test { .. } | Commands::Lsp { .. } | Commands::Setup { .. } => unreachable!(),
    };

    syntax_config::set_cosy_syntax(cosy_syntax);
    let binary_path = rosy(&source, output_dir, release, optimized)?;

    // Show update notice after transpilation (network has had time)
    update_handle.finish();

    // Run or copy the binary
    match cli.command {
        Commands::Run { .. } => {
            eprintln!("{BOLD}{CYAN}     Running{RESET} {}\n", source.display());

            let status = Command::new(&binary_path)
                .status()
                .with_context(|| format!("Failed to run binary at `{}`!", binary_path.display()))?;
            ensure!(
                status.success(),
                "Execution failed with exit code: {:?}",
                status.code()
            );
        }
        Commands::Build { .. } => {
            let destination = PathBuf::from(output_name.unwrap());
            std::fs::copy(&binary_path, &destination)
                .context("Failed to copy binary to current directory")?;
            eprintln!("  Binary written to {BOLD}{}{RESET}", destination.display());
        }
        Commands::Test { .. } | Commands::Lsp { .. } | Commands::Setup { .. } => unreachable!(),
    }

    Ok(())
}
