# Contributing to Patina

Thanks for your interest in contributing. This document covers how to get started.

## Development Setup

You need either:
- **Docker only** (recommended) — no local Rust or PHP needed
- **Local toolchain** — Rust stable + PHP dev headers + clang

### Docker (recommended)

```bash
git clone https://github.com/PeterBooker/patina.git
cd patina
make test       # Run Rust + PHP tests
make bench      # Run benchmarks
make build      # Build release .so for PHP 8.3
```

See the [Makefile](Makefile) for all available targets.

### Local toolchain

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install PHP dev headers + clang (Ubuntu/Debian)
sudo apt install php-dev libclang-dev pkg-config

# Build and test
cargo test --workspace
cargo build --release -p patina-ext
php -d extension=target/release/libpatina.so -r "echo patina_version();"
```

## Making Changes

1. Fork the repo and create a branch from `main`
2. Make your changes
3. Ensure all checks pass: `make check` (or manually: `cargo test`, `cargo clippy`, `cargo fmt`)
4. Submit a pull request

## Adding a WordPress Function

See [docs/ADDING-A-FUNCTION.md](docs/ADDING-A-FUNCTION.md) for the step-by-step process.

## Code Style

- Rust: `cargo fmt` (enforced in CI)
- Rust: `cargo clippy -- -D warnings` (enforced in CI)
- PHP: PSR-12 style

## Testing

Every function must have:
- Rust unit tests in `patina-core` (including fixture validation against WordPress)
- PHP extension tests comparing output to WordPress fixtures
- A fuzz target in `fuzz/`

Run everything: `make check`

## Reporting Issues

- Bug reports: include PHP version, architecture, and the WordPress function involved
- Feature requests: include which WordPress function and why it's a good candidate (call frequency, API simplicity)
