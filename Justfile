default:
    just --list

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all --check

check:
    cargo check --all-targets

lint:
    cargo clippy --all-targets -- -D warnings

test:
    cargo test

gate: fmt-check check lint test
