use anyhow::{Context, Result, ensure};
use clap::Subcommand;
use std::{fs, fs::write, path::PathBuf};

use super::{BOLD, DIM, GREEN, RESET};

#[derive(Subcommand)]
pub(crate) enum EditorTarget {
    /// Install the VS Code extension (syntax highlighting + LSP)
    Vscode,
    /// Install the Zed extension (language config + LSP setup)
    Zed,
}

// VS Code: package.json + thin client. language config is generated from
// the grammar at build time. vscode-languageclient is installed via npm.
const VSCODE_PACKAGE_JSON: &str = include_str!("../../assets/editors/vscode/package.json");
const VSCODE_LANG_CONFIG: &str = include_str!(concat!(
    env!("OUT_DIR"),
    "/vscode_language_configuration.json"
));
const VSCODE_EXTENSION_JS: &str = include_str!("../../assets/editors/vscode/src/extension.js");
const VSCODE_TM_GRAMMAR: &str =
    include_str!("../../assets/editors/vscode/syntaxes/rosy.tmLanguage.json");

// Zed extension files — embedded at build time.
// Unlike VS Code, Zed extensions need a WASM component, so we write the
// full extension source directory and the user installs it as a dev extension.
const ZED_EXTENSION_TOML: &str = include_str!("../../assets/editors/zed/extension.toml");
const ZED_CARGO_TOML: &str = include_str!("../../assets/editors/zed/Cargo.toml");
const ZED_LIB_RS: &str = include_str!("../../assets/editors/zed/src/lib.rs");
const ZED_CONFIG_TOML: &str = include_str!("../../assets/editors/zed/languages/rosy/config.toml");
const ZED_HIGHLIGHTS_SCM: &str = include_str!(concat!(env!("OUT_DIR"), "/highlights.scm"));

pub(crate) fn install_editor_extension(editor: &EditorTarget) -> Result<()> {
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

    let npm_status = std::process::Command::new("npm")
        .args(["install", "--omit=dev"])
        .current_dir(&ext_dir)
        .status()
        .context("Failed to run npm. Install Node.js so vscode-languageclient can be fetched.")?;
    ensure!(
        npm_status.success(),
        "npm install failed in {}",
        ext_dir.display()
    );

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
