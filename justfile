set shell := ["bash", "-cu"]
set dotenv-load := true

# List available recipes
default:
    @just --list

# Sync Python environment and build the Rust PyO3 extension in development mode
setup:
    uv sync
    uv run maturin develop --manifest-path crates/cull-core/Cargo.toml

# Rebuild only the Rust native module (release-optimized for file benchmarking)
build-core:
    uv run maturin develop --release --manifest-path crates/cull-core/Cargo.toml

# Run Clippy and Ruff linters across the workspace
lint:
    cargo clippy --manifest-path crates/cull-core/Cargo.toml --all-targets -- -D warnings
    uv run ruff check .
    uv run ruff format --check .

# Auto-format Rust and Python code
fmt:
    cargo fmt --manifest-path crates/cull-core/Cargo.toml
    uv run ruff format .
    uv run ruff check --fix .

# Run Rust and Python test suites
test:
    cargo test --manifest-path crates/cull-core/Cargo.toml
    uv run pytest

# Start the Litestar API server via Granian
serve-api:
    uv run granian --interface asgi backend.cull_api.app:app --host ${CULL_HOST:-127.0.0.1} --port ${CULL_PORT:-8000} --reload
