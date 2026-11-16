# meister — build/test/gate recipes. See docs/BUILDING.md for the
# primo build discipline these run under (cargo env, build lock).

build:
    cargo build

test:
    cargo test

clippy:
    cargo clippy --all-targets -- -D warnings

fmt:
    cargo fmt --check

# The full Rung gate sequence: format, strict clippy, all tests.
gate:
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo test

# Regenerate the SCIP code-intelligence index (index.scip at the
# repo root). Generated on demand; never committed (see
# docs/BUILDING.md §SCIP).
scip:
    rust-analyzer scip . --output index.scip
