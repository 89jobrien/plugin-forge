# plugin-forge

Build and verify agent harness plugins

## Development

- `just check`: run `cargo check --all-targets`
- `just lint`: run `cargo clippy --all-targets -- -D warnings`
- `just test`: run `cargo test`
- `just gate`: run format, lint, and test gates
