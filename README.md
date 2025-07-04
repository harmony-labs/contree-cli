# Contree CLI

[![Build Status](https://github.com/harmony-labs/contree-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/harmony-labs/contree-cli/actions)
[![Crates.io](https://img.shields.io/crates/v/contree-cli.svg)](https://crates.io/crates/contree-cli)
[![Docs.rs](https://docs.rs/contree-cli/badge.svg)](https://docs.rs/contree-cli)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

A Rust-based command-line utility to provide project context by scanning directories and optionally including relevant dependency files, with support for filtering and custom output.

## Quick Start

```bash
cargo install contree-cli
contree --help
```

## Overview

`contree` is designed to help developers quickly gather and inspect project files and dependencies, particularly useful for debugging or understanding project structure. It respects `.gitignore` and custom `.contreeignore` files, supports grep-style filtering, and can include dependency files referenced in error outputs (for Rust projects).

## Features

- **Directory Scanning**: Recursively scans a specified directory (or current working directory by default) for files.
- **File Filtering**: Supports grep-style pattern matching to filter files by content.
- **Explicit File Inclusion**: Allows specifying files to include regardless of filters or directory scope.
- **Dependency Analysis**: For Rust projects, extracts and includes dependency files referenced in error messages or related to specific types/macros.
- **Custom Output**: Outputs results to stdout or a specified file.
- **Pipeline Support**: Accepts piped input (e.g., from test output) and integrates it with the context generation.

## Installation

### Build and Install

#### Downloadable binary

1. Ensure you have `ubi` installed (`brew install ubi` on Mac)
2. `ubi --project harmony-labs/contree-cli --exe contree --in /usr/local/bin/`

   (`ubi --project harmony-labs/contree-cli --tag v0.1.4 --exe contree --in /usr/local/bin/` for specific version)

#### From source

*Prerequisites*

- [Rust](https://www.rust-lang.org/tools/install) (version 1.56 or later, as it uses the 2021 edition)
- [Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html)

1. Clone the repository or copy the project files to your local machine.
2. Navigate to the project directory:
   ```bash
   cd /path/to/contree-cli
   ```
3. Build the project:
   ```bash
   make build
   ```
4. Install the binary:
   ```bash
   make install
   ```
   This installs `contree` to your Cargo bin directory (typically `~/.cargo/bin`).

Alternatively, build and run directly without installation:
```bash
make run
```

## Usage

### Basic Command
Run `contree` in the current directory:
```bash
contree
```

### Options
- `-d, --dir <PATH>`: Specify the directory to scan (defaults to `.`).
- `-g, --grep <PATTERN>`: Filter files by content matching a pattern (plain text or `/regex/` for regex).
- `-i, --include <FILES>`: Comma-separated list of files to include (e.g., `file1.rs,file2.rs`).
- `-D, --include-deps`: Include dependency files referenced in errors (Rust projects only).
- `-o, --output <FILE>`: Write output to a file instead of stdout.

### Examples
1. Scan the current directory and filter files containing "transaction":
   ```bash
   contree --grep transaction
   ```
   Or with regex:
   ```bash
   contree --grep "/(D|d)atabase/"
   ```

2. Include specific files and output to a file:
   ```bash
   contree --include src/main.rs,lib.rs --output context.md
   ```

3. Pipe test output and include dependencies:
   ```bash
   cargo test | contree --include-deps
   ```

4. Use Makefile targets:
   ```bash
   make run-grep GREP=transaction
   make run-include INCLUDE=src/main.rs
   ```

## Project Structure

- **`src/main.rs`**: Main application logic.
- **`Cargo.toml`**: Rust package manifest with dependencies.
- **`Makefile`**: Build and run shortcuts.
- **`.contreeignore`**: Custom ignore file (e.g., excludes `Cargo.lock`).
- **`.gitignore`**: Standard Git ignore file (e.g., excludes `target/`).

### Dependencies
- `anyhow`: Error handling.
- `regex`: Parsing and filtering with regular expressions.
- `walkdir` & `ignore`: Directory traversal with ignore file support.
- `clap`: Command-line argument parsing.
- `atty`: Detects if input/output is a terminal.

## Development

### Build
```bash
make build
```

### Clean
```bash
make clean
```

### Release Build
```bash
make release
```

## Notes
- The tool assumes a Rust project when `--include-deps` is used and looks for `Cargo.toml` to confirm.
- Dependency file inclusion relies on `cargo tree` and the local Cargo registry (typically `~/.cargo/registry`).
- Binary files are skipped during grep filtering and marked as `[binary file]` in output.

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## Code of Conduct

This project adheres to the [Contributor Covenant](CODE_OF_CONDUCT.md). By participating, you are expected to uphold this code.

## Security

If you discover a security vulnerability, please see [SECURITY.md](SECURITY.md) for how to report it.

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for release history.

## License

This project is licensed under the terms of the [MIT License](LICENSE).

## Author

@mateodelnorte