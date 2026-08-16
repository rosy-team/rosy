//! # Rosy Package Manifest (`Rosy.toml`)
//!
//! A package directory is identified by a `Rosy.toml` at its root with the
//! shape:
//!
//! ```toml
//! [package]
//! name = "libcosy"
//! version = "1.0.0"
//! rosy_version = "^0.42"
//! ```
//!
//! - `name`: human-friendly package identifier; also used as the cache
//!   directory name for GITHUB sources.
//! - `version`: package's own semver. For PATH sources, a `MODULE` statement
//!   may pin this exactly — a mismatch is a hard error.
//! - `rosy_version`: a [semver requirement](https://docs.rs/semver) the
//!   running `rosy` binary must satisfy. Mismatches are hard errors so a
//!   library can refuse to load against an incompatible transpiler.
//!
//! Manifests are read at AST-construction time inside
//! [`Program::from_rule_with_includes`](crate::program::Program::from_rule_with_includes),
//! the same hook that resolves `INCLUDE`.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct RosyToml {
    pub package: PackageManifest,
}

#[derive(Debug, Deserialize)]
pub struct PackageManifest {
    pub name: String,
    pub version: String,
    pub rosy_version: String,
}

impl RosyToml {
    /// Parse a `Rosy.toml` from disk at `<package_dir>/Rosy.toml`.
    pub fn read_from(package_dir: &Path) -> Result<Self> {
        let manifest_path = package_dir.join("Rosy.toml");
        let raw = std::fs::read_to_string(&manifest_path).with_context(|| {
            format!(
                "Failed to read package manifest at '{}'",
                manifest_path.display()
            )
        })?;
        toml::from_str::<RosyToml>(&raw).with_context(|| {
            format!(
                "Failed to parse package manifest at '{}'",
                manifest_path.display()
            )
        })
    }

    /// Verify that the running `rosy` binary's version satisfies
    /// `package.rosy_version`. Returns a friendly error if not.
    pub fn check_rosy_version_compat(&self, current: &str) -> Result<()> {
        let req = semver::VersionReq::parse(&self.package.rosy_version).with_context(|| {
            format!(
                "Package '{}' has an invalid `rosy_version` requirement '{}'",
                self.package.name, self.package.rosy_version,
            )
        })?;
        let cur = semver::Version::parse(current).with_context(|| {
            format!("Internal: failed to parse current rosy version '{current}'")
        })?;
        if !req.matches(&cur) {
            bail!(
                "Package '{}' requires rosy {} but this binary is v{}",
                self.package.name,
                self.package.rosy_version,
                current,
            );
        }
        Ok(())
    }
}
