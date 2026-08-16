//! # Rosy Language Reference
//!
//! This is the complete reference for the Rosy programming language. A Rosy
//! program is a `BEGIN; ... END;` block containing [`statements`] that operate
//! on [`expressions`].
//!
//! ## Where to start
//!
//! - **Writing statements** (declarations, loops, I/O, etc.) → **[`statements`]**
//! - **Using expressions** (operators, functions, literals) → **[`expressions`]**
//!
//! Both modules have "Looking for something?" tables that link directly to
//! every language construct.

use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use crate::{
    ast::{CosyParser, FromRule, Rule},
    manifest::RosyToml,
    program::statements::Statement,
    transpile::*,
};
use anyhow::{Context, Error, Result, bail};
use pest::Parser;

/// Discriminator for the `MODULE` statement's source-type literal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleSourceType {
    /// `MODULE PATH "<dir>" [<version>];` — local directory.
    Path,
    /// `MODULE GITHUB "<owner>/<repo>" [<version>];` — git-cloned package.
    Github,
}

impl ModuleSourceType {
    fn label(self) -> &'static str {
        match self {
            ModuleSourceType::Path => "PATH",
            ModuleSourceType::Github => "GITHUB",
        }
    }
}

/// Parsed `MODULE` statement: source-type literal + path string + optional version pin.
#[derive(Debug)]
struct ModuleInfo {
    source_type: ModuleSourceType,
    path: String,
    version: Option<String>,
}

pub mod expressions;
pub mod statements;

/// Tracks INCLUDE resolution across one compilation unit.
///
/// Two distinct concerns share this struct because they share lookup paths:
///
/// * `in_progress` — files currently being parsed up the recursion stack.
///   A repeated INCLUDE of an in-progress file is a true cycle (A→B→A) and
///   surfaces as a `Circular INCLUDE detected` error.
///
/// * `completed` — files that have been fully resolved at least once. A
///   repeated INCLUDE of a completed file is silently skipped (no-op),
///   which mirrors Rust's `mod foo;` and Python's `import foo` semantics:
///   declarations inside the included file enter the program exactly once,
///   even when several leaf files all `INCLUDE` the same library.
///
/// Without `completed`, a library file that shares a header (e.g.
/// `libcosy/helpers/math.rosy` declaring `INCLUDE '../globals.rosy';` so
/// it stands alone for LSP analysis) would re-emit every VARIABLE in
/// globals when transitively pulled through `INCLUDE 'libcosy';`.
#[derive(Debug, Default)]
pub struct IncludeTracker {
    in_progress: HashSet<PathBuf>,
    completed: HashSet<PathBuf>,
}

#[derive(Debug)]
pub struct Program {
    pub statements: Vec<Statement>,
}
impl TranspileableStatement for Program {}
impl FromRule for Program {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Program>> {
        Program::from_rule_with_includes(pair, None, &mut IncludeTracker::default())
    }
}

impl Program {
    /// Parse a program, recursively resolving INCLUDE statements at the AST level.
    ///
    /// Each included file must be a complete `BEGIN; ... END;` program.
    /// Its statements are spliced into the parent at the INCLUDE site.
    /// Repeated INCLUDEs of the same file (across different parents) are
    /// idempotent — declarations enter the program exactly once.
    pub fn from_rule_with_includes(
        pair: pest::iterators::Pair<Rule>,
        source_path: Option<&Path>,
        tracker: &mut IncludeTracker,
    ) -> Result<Option<Program>> {
        let mut statements = Vec::new();

        for stmt in pair.into_inner() {
            if stmt.as_rule() == Rule::include_stmt {
                // Extract the path from the string literal inside `include_stmt`
                let include_path = Self::extract_include_path(&stmt)?;

                // Resolve relative to the including file's directory
                let base_dir = source_path.and_then(|p| p.parent());
                let resolved = if Path::new(&include_path).is_absolute() {
                    PathBuf::from(&include_path)
                } else {
                    let base = base_dir.ok_or_else(|| {
                        anyhow::anyhow!(
                            "Cannot resolve relative INCLUDE '{}' — source file path is unknown \
                             (hint: save the file to disk first)",
                            include_path,
                        )
                    })?;
                    base.join(&include_path)
                };

                // Resolve to a concrete `mod.rosy` file:
                //   (1) `resolved` is a regular file        → use it (current behavior)
                //   (2) `resolved` is a directory           → look for `<dir>/mod.rosy`
                //   (3) `resolved` doesn't exist            → still try `<resolved>/mod.rosy`
                //                                             (so `INCLUDE 'libcosy';` works
                //                                              before any `libcosy.rosy` exists)
                let canonical = match std::fs::canonicalize(&resolved) {
                    Ok(p) if p.is_file() => p,
                    Ok(p) if p.is_dir() => {
                        let mod_path = p.join("mod.rosy");
                        std::fs::canonicalize(&mod_path).with_context(|| {
                            format!(
                                "INCLUDE '{}' resolved to directory '{}' but no 'mod.rosy' was found inside.\n\
                                 Hint: create '{}/mod.rosy' or include a specific .rosy file.",
                                include_path,
                                p.display(),
                                p.display(),
                            )
                        })?
                    }
                    Ok(p) => bail!(
                        "INCLUDE '{}' resolved to '{}' which is neither a regular file nor a directory",
                        include_path,
                        p.display(),
                    ),
                    Err(_) => {
                        let mod_path = resolved.join("mod.rosy");
                        std::fs::canonicalize(&mod_path).with_context(|| {
                            format!(
                                "Failed to resolve INCLUDE path '{}' — tried '{}' (file) and '{}/mod.rosy' (directory module)",
                                include_path,
                                resolved.display(),
                                resolved.display(),
                            )
                        })?
                    }
                };

                Self::splice_resolved_file(canonical, &mut statements, tracker)?;
            } else if stmt.as_rule() == Rule::module_stmt {
                Self::process_module_stmt(&stmt, source_path, &mut statements, tracker)?;
            } else {
                let pair_input = stmt.as_str();
                if let Some(statement) = Statement::from_rule(stmt)
                    .with_context(|| format!("Failed to build statement from:\n{}", pair_input))?
                {
                    statements.push(statement);
                }
            }
        }

        Ok(Some(Program { statements }))
    }

    /// Read, parse, and splice a resolved canonical file into `statements`,
    /// updating `tracker`. Shared by INCLUDE and MODULE since both ultimately
    /// reduce to "treat the file's `BEGIN; ... END;` body as inlined here".
    fn splice_resolved_file(
        canonical: PathBuf,
        statements: &mut Vec<Statement>,
        tracker: &mut IncludeTracker,
    ) -> Result<()> {
        // Idempotency: a file that has already been fully parsed once
        // contributes its declarations exactly once. Subsequent INCLUDEs
        // are silent no-ops, so library files can safely INCLUDE their
        // own dependencies without producing duplicate VARIABLEs when
        // the program also INCLUDEs those dependencies via another path.
        if tracker.completed.contains(&canonical) {
            return Ok(());
        }

        // True cycle detection: only the active recursion stack counts.
        if tracker.in_progress.contains(&canonical) {
            let chain: Vec<String> = tracker
                .in_progress
                .iter()
                .map(|p| p.display().to_string())
                .collect();
            bail!(
                "Circular INCLUDE detected: {} → {}",
                chain.join(" → "),
                canonical.display()
            );
        }

        let included_source = std::fs::read_to_string(&canonical)
            .with_context(|| format!("Failed to read included file '{}'", canonical.display()))?;

        let mut pairs = CosyParser::parse(Rule::program, &included_source)
            .with_context(|| format!("Failed to parse included file '{}'", canonical.display()))?;

        let program_pair = pairs
            .next()
            .ok_or_else(|| anyhow::anyhow!("Empty parse result for '{}'", canonical.display()))?;

        tracker.in_progress.insert(canonical.clone());
        let included_program =
            Program::from_rule_with_includes(program_pair, Some(&canonical), tracker)?;
        tracker.in_progress.remove(&canonical);
        tracker.completed.insert(canonical.clone());

        if let Some(prog) = included_program {
            for mut s in prog.statements {
                if s.source_location.file.is_none() {
                    s.source_location.file = Some(canonical.clone());
                }
                statements.push(s);
            }
        }
        Ok(())
    }

    /// Resolve a `MODULE` statement to a package directory, validate its
    /// manifest, then splice the package's `mod.rosy` like an INCLUDE.
    fn process_module_stmt(
        stmt: &pest::iterators::Pair<Rule>,
        source_path: Option<&Path>,
        statements: &mut Vec<Statement>,
        tracker: &mut IncludeTracker,
    ) -> Result<()> {
        let info = Self::extract_module_info(stmt)?;

        // Step 1: locate the package directory (resolution rules differ per source type).
        let package_dir = match info.source_type {
            ModuleSourceType::Path => {
                let resolved = if Path::new(&info.path).is_absolute() {
                    PathBuf::from(&info.path)
                } else {
                    let base = source_path.and_then(|p| p.parent()).ok_or_else(|| {
                        anyhow::anyhow!(
                            "Cannot resolve relative MODULE PATH '{}' — source file path is unknown \
                             (hint: save the file to disk first)",
                            info.path,
                        )
                    })?;
                    base.join(&info.path)
                };
                std::fs::canonicalize(&resolved).with_context(|| {
                    format!(
                        "MODULE PATH '{}' could not be resolved (looked at '{}')",
                        info.path,
                        resolved.display(),
                    )
                })?
            }
            ModuleSourceType::Github => {
                // Version is required for GITHUB — it pins the tagged Release we grab.
                let version = info.version.as_deref().ok_or_else(|| {
                    anyhow::anyhow!(
                        "MODULE GITHUB '{}' requires a version (the tag of the Release to download)",
                        info.path,
                    )
                })?;

                // Cache key = "<repo>-<version>" so different versions coexist.
                let repo_name = info
                    .path
                    .rsplit('/')
                    .next()
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "MODULE GITHUB '{}' is not a valid 'owner/repo' identifier",
                            info.path
                        )
                    })?;
                let cache_dir = PathBuf::from(".rosy_output")
                    .join("packages")
                    .join(format!("{repo_name}-{version}"));

                if !cache_dir.exists() {
                    Self::fetch_github_release(&info.path, version, &cache_dir)?;
                }

                std::fs::canonicalize(&cache_dir).with_context(|| {
                    format!(
                        "Failed to canonicalize package cache directory '{}'",
                        cache_dir.display()
                    )
                })?
            }
        };

        // Step 2: read the package manifest.
        let manifest = RosyToml::read_from(&package_dir)?;

        // Step 3: announce what we're pulling in.
        // Cargo-style action line: bold-green "Grabbing" right-aligned
        // around column 12 to sit flush with main.rs's "Compiling" / "Finished".
        // Bold = \x1b[1m, bright green = \x1b[92m, reset = \x1b[0m.
        eprintln!(
            "\n\x1b[1m\x1b[92m    Grabbing\x1b[0m \x1b[1m{}\x1b[0m v{} from \x1b[36m{}\x1b[0m '{}'",
            manifest.package.name,
            manifest.package.version,
            info.source_type.label(),
            info.path,
        );

        // Step 4: enforce the package's `rosy_version` semver requirement.
        manifest.check_rosy_version_compat(env!("CARGO_PKG_VERSION"))?;

        // Step 5: for PATH, an explicit version on the MODULE statement must
        // match the manifest's `version` exactly. (GITHUB uses the version as
        // a git ref, so the match is the clone itself.)
        if matches!(info.source_type, ModuleSourceType::Path)
            && let Some(requested) = &info.version
            && requested != &manifest.package.version
        {
            bail!(
                "MODULE PATH '{}' requested version '{}' but package '{}' is at version '{}'",
                info.path,
                requested,
                manifest.package.name,
                manifest.package.version,
            );
        }

        // Step 6: behave like INCLUDE on the package's mod.rosy entry point.
        let mod_path = package_dir.join("mod.rosy");
        let canonical = std::fs::canonicalize(&mod_path).with_context(|| {
            format!(
                "Package '{}' is missing 'mod.rosy' at '{}'",
                manifest.package.name,
                mod_path.display(),
            )
        })?;
        Self::splice_resolved_file(canonical, statements, tracker)
    }

    /// Download and extract a GitHub Release source tarball into `dest`.
    ///
    /// Uses the public archive URL `https://github.com/<owner_repo>/archive/refs/tags/<version>.tar.gz`,
    /// which works for any tagged commit (whether or not a formal Release was
    /// created). The tarball's leading `<repo>-<verstrip>/` directory is
    /// stripped on the fly so files land directly inside `dest`.
    fn fetch_github_release(owner_repo: &str, version: &str, dest: &Path) -> Result<()> {
        use std::io::Read;

        let url = format!("https://github.com/{owner_repo}/archive/refs/tags/{version}.tar.gz");

        eprintln!("\x1b[1m\x1b[92m  Downloading\x1b[0m {url}");

        let agent = ureq::Agent::new_with_config(
            ureq::config::Config::builder()
                .timeout_global(Some(std::time::Duration::from_secs(60)))
                .build(),
        );
        let mut response = agent
            .get(&url)
            .header("User-Agent", "rosy-transpiler")
            .call()
            .with_context(|| format!("Failed to fetch GitHub release tarball '{url}'"))?;

        let status = response.status();
        if status != 200 {
            bail!(
                "GitHub returned HTTP {status} for '{url}' \
                 — check that '{owner_repo}' exists and tag '{version}' is published"
            );
        }

        let mut bytes = Vec::new();
        response
            .body_mut()
            .as_reader()
            .read_to_end(&mut bytes)
            .with_context(|| format!("Failed to read tarball body from '{url}'"))?;

        // Decompress + extract, stripping the GitHub-auto-added top-level dir.
        let gz = flate2::read::GzDecoder::new(bytes.as_slice());
        let mut archive = tar::Archive::new(gz);

        std::fs::create_dir_all(dest).with_context(|| {
            format!("Failed to create extraction directory '{}'", dest.display())
        })?;

        for entry in archive
            .entries()
            .with_context(|| format!("Failed to read entries from tarball '{url}'"))?
        {
            let mut entry =
                entry.with_context(|| format!("Corrupt tar entry in tarball '{url}'"))?;
            let entry_path = entry
                .path()
                .with_context(|| format!("Tar entry has invalid path in '{url}'"))?
                .into_owned();

            // Strip the leading dir component (e.g. "repo-1.0.0/").
            let stripped: PathBuf = entry_path.components().skip(1).collect();
            if stripped.as_os_str().is_empty() {
                continue;
            }
            let target = dest.join(stripped);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("Failed to create '{}'", parent.display()))?;
            }
            entry
                .unpack(&target)
                .with_context(|| format!("Failed to write '{}'", target.display()))?;
        }

        Ok(())
    }

    /// Extract the source-type literal, path string, and optional version
    /// string from a `module_stmt` pest pair.
    fn extract_module_info(pair: &pest::iterators::Pair<Rule>) -> Result<ModuleInfo> {
        // module_stmt = { ^"MODULE" ~ module_source_type ~ string ~ string? ~ semicolon }
        let mut inner = pair.clone().into_inner();

        let source_type_pair = inner
            .next()
            .filter(|p| p.as_rule() == Rule::module_source_type)
            .ok_or_else(|| anyhow::anyhow!("MODULE statement missing source type"))?;
        let source_type = match source_type_pair.as_str().to_uppercase().as_str() {
            "PATH" => ModuleSourceType::Path,
            "GITHUB" => ModuleSourceType::Github,
            other => bail!("Unknown MODULE source type '{}'", other),
        };

        let path = Self::string_pair_to_owned(
            inner
                .next()
                .filter(|p| p.as_rule() == Rule::string)
                .ok_or_else(|| anyhow::anyhow!("MODULE statement missing path string"))?,
        )?;

        let version = match inner.next() {
            Some(p) if p.as_rule() == Rule::string => Some(Self::string_pair_to_owned(p)?),
            _ => None,
        };

        Ok(ModuleInfo {
            source_type,
            path,
            version,
        })
    }

    /// Strip the surrounding quotes from a `string` rule pair (handling both
    /// `"..."` and `'...'` forms, with the standard `''` → `'` unescape).
    fn string_pair_to_owned(string_pair: pest::iterators::Pair<Rule>) -> Result<String> {
        let inner = string_pair
            .into_inner()
            .next()
            .ok_or_else(|| anyhow::anyhow!("Empty string literal"))?;
        let raw = inner.as_str();
        let body = &raw[1..raw.len() - 1];
        Ok(if inner.as_rule() == Rule::old_string {
            body.replace("''", "'")
        } else {
            body.to_string()
        })
    }

    /// Extract the file path string from an `include_stmt` pair.
    fn extract_include_path(pair: &pest::iterators::Pair<Rule>) -> Result<String> {
        // include_stmt = { ^"INCLUDE" ~ string ~ semicolon }
        // string = { new_string | old_string }
        // new_string = @{ "\"" ~ ... ~ "\"" }
        // old_string = @{ "\'" ~ ... ~ "\'" }
        let string_pair = pair
            .clone()
            .into_inner()
            .find(|p| p.as_rule() == Rule::string)
            .ok_or_else(|| anyhow::anyhow!("INCLUDE statement missing path string"))?;

        let inner = string_pair
            .into_inner()
            .next()
            .ok_or_else(|| anyhow::anyhow!("INCLUDE path string is empty"))?;

        let raw = inner.as_str();
        // Strip surrounding quotes (first and last char)
        let path = &raw[1..raw.len() - 1];

        // For old_string (single-quoted), unescape ''  → '
        let path = if inner.as_rule() == Rule::old_string {
            path.replace("''", "'")
        } else {
            path.to_string()
        };

        Ok(path)
    }
}

impl Transpile for Program {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut serialization = Vec::new();
        let mut errors = Vec::new();
        for statement in &self.statements {
            match statement.transpile(context) {
                Ok(output) => {
                    serialization.push(output.serialization);
                }
                Err(stmt_errors) => {
                    for e in stmt_errors {
                        errors.push(e.context("...while transpiling a top-level statement"));
                    }
                }
            }
        }

        if errors.is_empty() {
            Ok(TranspilationOutput {
                serialization: serialization.join("\n"),
                requested_variables: BTreeSet::new(),
                ..Default::default()
            })
        } else {
            Err(errors)
        }
    }
}

#[cfg(test)]
mod include_resolution_tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn parse_and_resolve(source: &str, source_path: &Path) -> Result<Program> {
        let mut pairs = CosyParser::parse(Rule::program, source)
            .map_err(|e| anyhow::anyhow!("parse error: {e}"))?;
        let pair = pairs.next().ok_or_else(|| anyhow::anyhow!("empty parse"))?;
        Program::from_rule_with_includes(pair, Some(source_path), &mut IncludeTracker::default())?
            .ok_or_else(|| anyhow::anyhow!("from_rule_with_includes returned None"))
    }

    fn write(dir: &Path, name: &str, contents: &str) -> PathBuf {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, contents).unwrap();
        path
    }

    /// Baseline: existing behavior — INCLUDE points at a literal `.rosy` file.
    #[test]
    fn include_resolves_literal_file() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), "helper.rosy", "BEGIN;\nEND;\n");
        let main = write(
            tmp.path(),
            "main.rosy",
            "BEGIN;\nINCLUDE 'helper.rosy';\nEND;\n",
        );
        let src = fs::read_to_string(&main).unwrap();
        parse_and_resolve(&src, &main).expect("literal-file include should succeed");
    }

    /// New behavior: INCLUDE points at a directory; we look for `<dir>/mod.rosy`.
    #[test]
    fn include_resolves_directory_with_modrosy() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), "libcosy/mod.rosy", "BEGIN;\nEND;\n");
        let main = write(
            tmp.path(),
            "main.rosy",
            "BEGIN;\nINCLUDE 'libcosy';\nEND;\n",
        );
        let src = fs::read_to_string(&main).unwrap();
        parse_and_resolve(&src, &main).expect("directory include should resolve to mod.rosy");
    }

    /// Directory exists but has no `mod.rosy` — the error must mention both attempts.
    #[test]
    fn include_directory_without_modrosy_errors() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join("libcosy")).unwrap();
        let main = write(
            tmp.path(),
            "main.rosy",
            "BEGIN;\nINCLUDE 'libcosy';\nEND;\n",
        );
        let src = fs::read_to_string(&main).unwrap();
        let err = parse_and_resolve(&src, &main).unwrap_err();
        let chain: Vec<String> = err.chain().map(|e| e.to_string()).collect();
        let joined = chain.join(" :: ");
        assert!(
            joined.contains("mod.rosy"),
            "error chain should mention 'mod.rosy': {joined}"
        );
    }

    /// Neither a file nor a directory at the path — error names both attempted paths.
    #[test]
    fn include_nonexistent_errors() {
        let tmp = TempDir::new().unwrap();
        let main = write(tmp.path(), "main.rosy", "BEGIN;\nINCLUDE 'nope';\nEND;\n");
        let src = fs::read_to_string(&main).unwrap();
        let err = parse_and_resolve(&src, &main).unwrap_err();
        let joined: String = err
            .chain()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join(" :: ");
        assert!(
            joined.contains("nope"),
            "error should mention the missing 'nope': {joined}"
        );
        assert!(
            joined.contains("mod.rosy") || joined.contains("directory module"),
            "error should mention the directory-fallback attempt: {joined}"
        );
    }

    /// Relative paths inside a directory's `mod.rosy` resolve relative to that file's dir,
    /// so a parent `mod.rosy` can `INCLUDE 'sibling';` to load `sibling/mod.rosy`.
    #[test]
    fn include_relative_path_through_directory() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), "libcosy/physics/mod.rosy", "BEGIN;\nEND;\n");
        write(
            tmp.path(),
            "libcosy/mod.rosy",
            "BEGIN;\nINCLUDE 'physics';\nEND;\n",
        );
        let main = write(
            tmp.path(),
            "main.rosy",
            "BEGIN;\nINCLUDE 'libcosy';\nEND;\n",
        );
        let src = fs::read_to_string(&main).unwrap();
        parse_and_resolve(&src, &main)
            .expect("nested directory include should resolve transitively");
    }

    /// Including the same file twice (via different INCLUDE chains) is a
    /// silent no-op the second time — declarations enter the program once.
    /// This protects libraries that INCLUDE their own dependencies for
    /// standalone analysis.
    #[test]
    fn include_idempotent_double_include() {
        let tmp = TempDir::new().unwrap();
        // header.rosy declares a single global; it must NOT be redeclared
        // when reached via two different INCLUDE chains.
        write(
            tmp.path(),
            "header.rosy",
            "BEGIN;\nVARIABLE (RE) SHARED;\nEND;\n",
        );
        // a.rosy and b.rosy both include header.rosy …
        write(
            tmp.path(),
            "a.rosy",
            "BEGIN;\nINCLUDE 'header.rosy';\nEND;\n",
        );
        write(
            tmp.path(),
            "b.rosy",
            "BEGIN;\nINCLUDE 'header.rosy';\nEND;\n",
        );
        // … and main.rosy includes both, so header.rosy would otherwise be
        // spliced twice → "VARIABLE 'SHARED' already defined".
        let main = write(
            tmp.path(),
            "main.rosy",
            "BEGIN;\nINCLUDE 'a.rosy';\nINCLUDE 'b.rosy';\nEND;\n",
        );
        let src = fs::read_to_string(&main).unwrap();
        let prog = parse_and_resolve(&src, &main)
            .expect("idempotent INCLUDE should silently skip the second visit");
        // header.rosy contributes exactly one statement (`VARIABLE SHARED`).
        // a.rosy and b.rosy each contribute zero of their own. Without
        // idempotency the count would be 2 (one per chain).
        assert_eq!(
            prog.statements.len(),
            1,
            "expected exactly one statement after idempotent INCLUDE, got {}",
            prog.statements.len()
        );
    }

    /// Circular detection still fires when the cycle goes through directory modules.
    #[test]
    fn include_circular_through_directories() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), "a/mod.rosy", "BEGIN;\nINCLUDE '../b';\nEND;\n");
        write(tmp.path(), "b/mod.rosy", "BEGIN;\nINCLUDE '../a';\nEND;\n");
        let main = write(tmp.path(), "main.rosy", "BEGIN;\nINCLUDE 'a';\nEND;\n");
        let src = fs::read_to_string(&main).unwrap();
        let err = parse_and_resolve(&src, &main).unwrap_err();
        let joined: String = err
            .chain()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join(" :: ");
        assert!(
            joined.contains("Circular INCLUDE"),
            "error chain should report circular INCLUDE: {joined}"
        );
    }
}
