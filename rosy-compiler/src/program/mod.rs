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
    ast::{FromRule, Rule},
    program::statements::Statement,
    transpile::*,
};

pub mod expressions;
pub mod manifest;
pub mod statements;
pub mod syntax_config;
use anyhow::{Context, Error, Result, bail};
use manifest::RosyToml;
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
        crate::syntax_config::with_path(source_path, || {
            Self::from_rule_with_includes_inner(pair, source_path, tracker)
        })
    }

    fn from_rule_with_includes_inner(
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

                let canonical = Self::resolve_include_file(&resolved, &include_path)?;

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

    fn is_include_source(path: &Path) -> bool {
        match path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_ascii_lowercase())
        {
            Some(ext) if ext == "fox" || ext == "rosy" => true,
            Some(_) => false,
            None => false,
        }
    }

    /// Prefer `.fox` / `.rosy` over a same-stem binary (`INCLUDE 'COSY'` vs `./cosy`).
    fn resolve_include_file(resolved: &Path, include_path: &str) -> Result<PathBuf> {
        let mut candidates = Vec::new();
        if Self::is_include_source(resolved) {
            candidates.push(resolved.to_path_buf());
        }
        if let Some(parent) = resolved.parent() {
            if let Some(stem) = resolved.file_name() {
                for ext in [".fox", ".FOX", ".rosy", ".ROSY"] {
                    candidates.push(
                        parent.join(format!("{}{ext}", stem.to_string_lossy().to_uppercase())),
                    );
                    candidates.push(
                        parent.join(format!("{}{ext}", stem.to_string_lossy().to_lowercase())),
                    );
                    candidates.push(parent.join(format!("{}{ext}", stem.to_string_lossy())));
                }
            }
        }
        if resolved.is_file() {
            candidates.push(resolved.to_path_buf());
        }
        candidates.push(resolved.join("mod.rosy"));

        let candidates_for_printing = candidates
            .iter()
            .map(|c| c.display().to_string())
            .collect::<Vec<String>>();
        println!("candidates: {}", candidates_for_printing.join(", "));

        let mut seen = HashSet::new();
        for cand in candidates {
            if !seen.insert(cand.clone()) {
                continue;
            }
            if cand.is_file() && (Self::is_include_source(&cand) || cand.ends_with("mod.rosy")) {
                return std::fs::canonicalize(&cand).with_context(|| {
                    format!("Failed to canonicalize INCLUDE '{}'", cand.display())
                });
            }
        }

        bail!(
            "Failed to resolve INCLUDE path '{}' — tried '{}",
            include_path,
            candidates_for_printing.join(", "),
        )
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

        tracker.in_progress.insert(canonical.clone());
        let included_program = crate::syntax_config::with_path(Some(&canonical), || {
            let mut pairs = crate::ast::parse_include(&included_source).with_context(|| {
                format!("Failed to parse included file '{}'", canonical.display())
            })?;
            let program_pair = pairs.next().ok_or_else(|| {
                anyhow::anyhow!("Empty parse result for '{}'", canonical.display())
            })?;
            Program::from_rule_with_includes(program_pair, Some(&canonical), tracker)
        })?;
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
mod tests {
    use super::*;
    use crate::ast;
    use crate::program::statements::StmtKind;

    #[test]
    fn nested_procedure_keeps_source_file() {
        let src =
            "BEGIN;\nPROCEDURE OUTER;\nPROCEDURE INNER;\nENDPROCEDURE;\nENDPROCEDURE;\nEND;\n";
        let pair = ast::parse_source(src).unwrap().next().unwrap();
        let path = Path::new("/tmp/nested.rosy");
        let prog =
            Program::from_rule_with_includes(pair, Some(path), &mut IncludeTracker::default())
                .unwrap()
                .unwrap();
        let outer = &prog.statements[0];
        assert_eq!(outer.source_location.file.as_deref(), Some(path));
        let StmtKind::Procedure(p) = &outer.inner else {
            panic!("expected procedure");
        };
        assert_eq!(p.name, "OUTER");
        assert_eq!(p.body[0].source_location.file.as_deref(), Some(path));
    }
}
