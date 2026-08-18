mod cli;

use anyhow::{Context, Result, ensure};
use clap::{Parser as ClapParser, Subcommand};
use std::{path::PathBuf, process::Command};

use cli::setup::EditorTarget;
use cli::{BOLD, CYAN, RESET};


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

fn main() -> Result<()> {
    let cli = Cli::parse();

    if matches!(&cli.command, Commands::Lsp { .. }) {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to create tokio runtime");
        rt.block_on(rosy::lsp::run());
        return Ok(());
    }

    if let Commands::Setup { editor } = &cli.command {
        return cli::setup::install_editor_extension(editor);
    }

    if let Commands::Test {
        filter,
        release,
        bless,
    } = &cli.command
    {
        return cli::test::run_construct_tests(filter.as_deref(), *release, *bless);
    }

    let (source, output_dir, release, optimized, output_name) = match &cli.command {
        Commands::Run {
            source,
            output_dir,
            release,
            optimized,
        } => (
            source.clone(),
            output_dir.clone(),
            *release || *optimized,
            *optimized,
            None,
        ),
        Commands::Build {
            source,
            output,
            output_dir,
            release,
            optimized,
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
                Some(name),
            )
        }
        Commands::Test { .. } | Commands::Lsp { .. } | Commands::Setup { .. } => unreachable!(),
    };

    let binary_path = cli::compile::rosy(&source, output_dir, release, optimized)?;

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
