# CLAUDE.md

Guidance for working on `plugin-forge`.

## Project

Build and verify agent harness plugins

## Commands

- `just fmt`: format Rust code
- `just fmt-check`: verify Rust formatting
- `just check`: run `cargo check --all-targets`
- `just lint`: run `cargo clippy --all-targets -- -D warnings`
- `just test`: run `cargo test`
- `just gate`: run the full local validation gate

## Conventions

- Rust edition 2024.
- Treat warnings as errors with `cargo clippy -- -D warnings`.
- Prefer small modules and explicit domain boundaries.
- Do not commit secrets or machine-local files.

@OPAVS.md
