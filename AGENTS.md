# Rosy

Rosy transpiles ROSY source code (.rosy) into self-contained Rust executables for beam physics and differential algebra. It is a modern reimplementation of the COSY INFINITY language.

## Build & Test

```bash
cargo build --release                              # Build transpiler
cargo test                                         # Run unit tests
cargo run --bin rosy -- run examples/basic.rosy     # Run a ROSY script
cargo run --bin rosy -- build examples/basic.rosy   # Build standalone binary
```

## Type System
RE (f64), ST (String), LO (bool), CM (Complex64), VE (Vec<f64>), DA (Taylor series), CD (Complex DA)

Multi-dimensional arrays: `(RE 2 2)` = `Vec<Vec<f64>>` (initialized as a 2x2 vec)

## Versioning

Uses [Semantic Versioning](https://semver.org/). The version is in `rosy-cli/Cargo.toml` (CLI) and `rosy-compiler/Cargo.toml` (keep them in sync).

- **Every commit to `master`** must bump the version in `rosy-cli/Cargo.toml`
  - Patch bump (0.1.0 -> 0.1.1): bug fixes, small improvements
  - Minor bump (0.1.0 -> 0.2.0): new language constructs, features, breaking changes, or changes where doc rebuilds are desired
  - Major bump: reserved for 1.0 stable release
- **Releases are automatic**: a version bump in a crate's `Cargo.toml` pushed to `master` publishes that crate to crates.io (skipped if that version already exists). `rosy-cli` also builds binaries, tags `vX.Y.Z`, and creates a GitHub Release if that tag is new.
