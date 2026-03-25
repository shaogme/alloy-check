# Alloy-Check

`alloy-check` is a strict, workspace-level linting and code quality tool designed to enforce a standardized set of rules for Rust projects. It goes beyond `cargo fmt` and `cargo clippy` by parsing the Abstract Syntax Tree (AST) to verify structural layout, safety bounds, naming regulations, and metadata correctness.

## Features

This tool implements rule definitions found in [SPECIFICATION.md](SPECIFICATION.md), including:

1. **Standard Cargo Checks**: Verifies `cargo run fmt` and `cargo clippy` for warnings or errors.
2. **Metadata Checks**: Enforces `edition = "2024"` and ensures crates have a `description` and `license`.
3. **AST Traversals & Customs Rules**: 
   - Restricts fully qualified imports (`PATH001`).
   - Ensures `mod` and `use` are placed at the beginning of respective blocks (`PATH002`).
   - Enforces the modern Rust module structure by blocking `mod.rs` (`PATH003`).
   - Limits identifier character lengths (`ID001`) and function sizes (`FUNC001`, `FUNC002`).
   - Detects useless function aliases (`FUNC003`).
   - Prevents use of `unwrap()`, `expect()`, `panic!()` in non-test functions (`SAFE001`, `SAFE002`).
   - Requires a `// SAFETY:` block comment above `unsafe` usage (`SAFE003`).
   - Mandates rustdoc (`///`) for all `pub` APIs (`DOC001`).

## Installation

### Local Installation
You can run `alloy-check` inside any cargo project.

```sh
cargo install --path .
```

### GitHub Action
To use `alloy-check` in your GitHub Workflows:

```yaml
- name: Setup Alloy Check
  uses: shaogme/alloy-check@main
```

Then you can use the `alloy-check` command directly in subsequent steps.

## Usage

Navigate to your Rust workspace or crate, and run:

```sh
alloy-check
```

### Options

```text
Usage: alloy-check [OPTIONS]

Options:
  -p, --path <PATH>  Path to the workspace root (defaults to current directory) [default: .]
  -v, --verbose      Verbose output
  -h, --help         Print help
  -V, --version      Print version
```

## Disabling Checks

In case certain generated directories or legacy code need exceptions, `alloy-check` automatically skips `target/` and any paths containing the word `generated`. 
You can also manually add ignore globs in your `Cargo.toml`:

```toml
[package.metadata.alloy-check]
ignore = ["tests/**/*", "src/legacy.rs"]
```

## LICENSE

This project is licensed under the MIT License. See the [LICENSE](./LICENSE) file for details.