# Rosy

A modern transpiler for the Rosy scientific programming language, designed for beam physics and differential algebra applications.

Rosy transpiles source code into self-contained, native Rust executables — optimized native code with zero runtime dependencies.

## Language Documentation

The complete Rosy language reference — every operator, function, statement, and type — is in the **[Rustdoc documentation](https://docs.rs/rosy-compiler/latest/rosy_compiler/)**.

## Example

```
BEGIN;
    VARIABLE (RE) x;
    VARIABLE (RE) y;
    x := 3;
    y := SIN(x) + 1;
    WRITE 6 'y = ' ST(y);
END;
```

```
$ rosy run example.rosy
y =  1.141120008059867E-001
```

## Installation

```bash
cargo install rosy-cli
```

### From source (recommended)

Requires the [Rust nightly toolchain](https://rustup.rs/) (needed for `--optimized` SIMD support):

```bash
# Install Rust (if you don't have it)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Switch to nightly (required for --optimized builds)
rustup default nightly

# Build and install Rosy
git clone https://github.com/rosy-team/rosy.git
cd rosy
cargo install --path rosy-cli
```

To update:

```bash
cd rosy
git pull
rustup update nightly
cargo install --path rosy-cli
```

### Migrating from Rust Stable to Nightly

If you previously installed Rosy with Rust stable, switch to nightly for `--optimized` support:

```bash
rustup default nightly
cd rosy && git pull
cargo install --path rosy-cli --force
```

### NIU Metis Quick Start

```bash
# First-time setup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
rustup default nightly
git clone https://github.com/rosy-team/rosy.git
cd rosy && cargo install --path rosy-cli

# For MPI programs (PLOOP)
module load openmpi/openmpi-5.0.7-gcc-14.2.0-cuda-12.8

# Run a program
rosy run examples/basic.rosy
rosy build examples/basic.rosy --optimized -o my_program
```

To update:

```bash
cd ~/rosy && git pull
rustup update nightly
cargo install --path rosy-cli --force
```

### From GitHub Releases

Prebuilt binaries for Linux (x86_64) and macOS (x86_64, aarch64) are available on the [Releases page](https://github.com/rosy-team/rosy/releases/latest).

### Using Nix Flakes

```bash
nix develop   # Enters a shell with nightly Rust + all dependencies
```

## Quick Start

```bash
rosy run examples/basic.rosy                   # run directly
rosy build examples/basic.rosy -o out          # build a binary
rosy build examples/basic.rosy --release       # release build
rosy build examples/basic.rosy --optimized     # max performance (recommended)
```

## Build Modes

Use `--optimized` for any real computation. It produces significantly faster binaries at the cost of longer compile times.

| Flag | Compile time | Runtime | Use case |
|------|-------------|---------|----------|
| *(none)* | ~1s | Slowest | Syntax checking, debugging |
| `--release` | ~2s | Fast | Quick iteration during development |
| **`--optimized`** | **~5s** | **Fastest** | **Production runs, benchmarks, beam physics** |

`--optimized` enables:
- **LTO** (link-time optimization) — whole-program optimization across all crate boundaries
- **Single codegen unit** — allows inlining of DA hot paths across `taylor/` and `intrinsics/` modules
- **SIMD DA operations** — `portable_simd` acceleration for differential algebra (requires nightly Rust)
- **`panic = abort`** — eliminates unwinding overhead

> **Recommendation:** Always use `--optimized` for actual physics runs. The extra ~3s of compile time is negligible compared to the runtime savings on any non-trivial computation. Reserve `--release` for rapid edit-run cycles during development.
> 
## MPI Support (`PLOOP`)

Programs using `PLOOP` require an MPI implementation and LLVM/Clang at compile time (the transpiler itself does not):

- **Ubuntu/Debian**: `sudo apt install libopenmpi-dev openmpi-bin libclang-dev`
- **Fedora**: `sudo dnf install openmpi-devel clang-devel`
- **macOS**: `brew install open-mpi llvm`
- **NIU Metis**: `module load openmpi/openmpi-5.0.7-gcc-14.2.0-cuda-12.8`

## Editor Support

For setup instructions, run either:
- `rosy setup zed`
- `rosy setup vscode`

## License

MIT. See `LICENSE`.
